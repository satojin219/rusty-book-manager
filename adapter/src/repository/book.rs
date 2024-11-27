use async_trait::async_trait;
use derive_new::new;
use kernel::model::book::{
    event::{CreateBook, UpdateBook},
    Book, BookListOptions,
};
use kernel::model::{
    id::{BookId, UserId},
    {book::event::DeleteBook, list::PaginatedList},
};
use kernel::repository::book::BookRepository;
use shared::error::{AppError, AppResult};

use crate::database::model::book::{BookRow, PaginatedBookRow};
use crate::database::ConnectionPool;

#[derive(new)]
pub struct BookRepositoryImpl {
    db: ConnectionPool,
}

#[async_trait]
impl BookRepository for BookRepositoryImpl {
    async fn create(&self, event: CreateBook, user_id: UserId) -> AppResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO  books (title, author, isbn, description, user_id)
            VALUES ($1, $2, $3, $4,$5)
            "#,
            event.title,
            event.author,
            event.isbn,
            event.description,
            user_id as _,
        )
        .execute(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?;

        Ok(())
    }

    async fn find_all(&self, option: BookListOptions) -> AppResult<PaginatedList<Book>> {
        let BookListOptions { limit, offset } = option;

        let rows: Vec<PaginatedBookRow> = sqlx::query_as!(
            PaginatedBookRow,
            r#"
            SELECT
                COUNT(*) OVER() as "total!",
                b.book_id AS id
            FROM books AS b
            ORDER BY b.created_at DESC
            LIMIT $1
            OFFSET $2
          "#,
            limit,
            offset,
        )
        .fetch_all(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?;

        let total = rows.first().map(|r| r.total).unwrap_or_default(); //レコードが一つもないときはtotalも0になる
        let book_ids = rows.into_iter().map(|r| r.id).collect::<Vec<BookId>>();

        let rows: Vec<BookRow> = sqlx::query_as!(
            BookRow,
            r#"
            SELECT
                b.book_id AS book_id,
                b.title AS title,
                b.author AS author,
                b.isbn AS isbn,
                b.description AS description,
                u.user_id AS owned_by,
                u.name AS owner_name
            FROM books AS b
            INNER JOIN users AS u USING(user_id)
            WHERE b.book_id IN (SELECT * FROM UNNEST($1::UUID[]))
            ORDER BY b.created_at DESC
            "#,
            &book_ids as _,
        )
        .fetch_all(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?;

        let items = rows.into_iter().map(Book::from).collect();

        Ok(PaginatedList {
            total,
            limit,
            offset,
            items,
        })
    }

    async fn find_by_id(&self, book_id: BookId) -> AppResult<Option<Book>> {
        let row: Option<BookRow> = sqlx::query_as!(
            BookRow,
            r#"
        SELECT
            b.book_id AS book_id,
            b.title AS title,
            b.author AS author,
            b.isbn AS isbn,
            b.description AS description,
            u.user_id AS owned_by,
            u.name AS owner_name
        FROM books AS b
        INNER JOIN users AS u USING(user_id)
        WHERE book_id = $1
        "#,
            book_id as _, //query_as!マクロによるコンパイル時の型チェックを無効化
        )
        .fetch_optional(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?;

        Ok(row.map(Book::from))
    }

    async fn update(&self, event: UpdateBook) -> AppResult<()> {
        let res = sqlx::query!(
            r#"
        UPDATE books
        SET
            title = $1,
            author = $2,
            isbn = $3,
            description = $4
        WHERE book_id = $5
        AND user_id = $6
        "#,
            event.title,
            event.author,
            event.isbn,
            event.description,
            event.book_id as _,
            event.requested_user as _,
        )
        .execute(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?;

        if res.rows_affected() < 1 {
            return Err(AppError::EntityNotFound("Specified boook not found".into()));
        }
        Ok(())
    }

    async fn delete(&self, event: DeleteBook) -> AppResult<()> {
        let res = sqlx::query!(
            r#"
        DELETE FROM books
        WHERE book_id = $1
        AND user_id = $2
        "#,
            event.book_id as _,
            event.requested_user as _,
        )
        .execute(self.db.inner_ref())
        .await
        .map_err(AppError::SpecificOperationError)?;

        if res.rows_affected() < 1 {
            return Err(AppError::EntityNotFound("Specified book not found".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{
        book::BookRepositoryImpl, checkout::CheckoutRepositoryImpl, user::UserRepositoryImpl,
    };
    use chrono::Utc;
    use kernel::{
        model::{
            checkout::event::{CreateCheckout, UpdateReturned},
            id::UserId,
            user::event::CreateUser,
        },
        repository::{checkout::CheckoutRepository, user::UserRepository},
    };
    use std::str::FromStr;

    #[sqlx::test]
    async fn test_register_book(pool: sqlx::PgPool) -> anyhow::Result<()> {
        // 蔵書のデータを追加・取得するためにはユーザー情報がないといけないため
        // テストコードのほうでもロールおよびユーザー情報を追加するコードを足した。
        // テストコードで、このようなデータベースにあらかじめデータを追加しておくために
        // fixture という機能が便利であるが、次章で解説するためここでは愚直な実装としておく。
        sqlx::query!(r#"INSERT INTO roles(name) VALUES ('Admin'), ('User');"#)
            .execute(&pool)
            .await?;
        let user_repo = UserRepositoryImpl::new(ConnectionPool::new(pool.clone()));
        let repo = BookRepositoryImpl::new(ConnectionPool::new(pool.clone()));
        let user = user_repo
            .create(CreateUser {
                name: "Test User".into(),
                email: "test@example.com".into(),
                password: "test_password".into(),
            })
            .await?;
        let book = CreateBook {
            title: "Test Title".into(),
            author: "Test Author".into(),
            isbn: "Test ISBN".into(),
            description: "Test Description".into(),
        };
        repo.create(book, user.id).await?;
        // find_all を実行するためには BookListOptions 型の値が必要なので作る。
        let options = BookListOptions {
            limit: 20,
            offset: 0,
        };
        let res = repo.find_all(options).await?;
        assert_eq!(res.items.len(), 1);
        let book_id = res.items[0].id;
        let res = repo.find_by_id(book_id).await?;
        assert!(res.is_some());
        let Book {
            id,
            title,
            author,
            isbn,
            description,
            owner,
            ..
        } = res.unwrap();
        assert_eq!(id, book_id);
        assert_eq!(title, "Test Title");
        assert_eq!(author, "Test Author");
        assert_eq!(isbn, "Test ISBN");
        assert_eq!(description, "Test Description");
        assert_eq!(owner.name, "Test User");
        Ok(())
    }

    #[sqlx::test(fixtures("common", "book"))]
    async fn test_update_book(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let repo = BookRepositoryImpl::new(ConnectionPool::new(pool.clone()));
        // 2. fixtures/book.sql で作成済みの書籍を取得
        let book_id = BookId::from_str("9890736e-a4e4-461a-a77d-eac3517ef11b").unwrap();
        let book = repo.find_by_id(book_id).await?.unwrap();
        const NEW_AUTHOR: &str = "更新後の著者名";
        assert_ne!(book.author, NEW_AUTHOR);

        // 3. 書籍の更新用のパラメータを作成し、更新を行う
        let update_book = UpdateBook {
            book_id: book.id,
            title: book.title,
            author: NEW_AUTHOR.into(), // ここが差分
            isbn: book.isbn,
            description: book.description,
            requested_user: UserId::from_str("5b4c96ac-316a-4bee-8e69-cac5eb84ff4c").unwrap(),
        };
        repo.update(update_book).await.unwrap();

        // 4. 更新後の書籍を取得し、期待通りに更新されていることを検証する
        let book = repo.find_by_id(book_id).await?.unwrap();
        assert_eq!(book.author, NEW_AUTHOR);

        Ok(())
    }

    #[sqlx::test(fixtures("common", "book"))]
    async fn test_delete_book(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let repo = BookRepositoryImpl::new(ConnectionPool::new(pool.clone()));
        let book_id = BookId::from_str("9890736e-a4e4-461a-a77d-eac3517ef11b")?;

        repo.delete(DeleteBook {
            book_id,
            requested_user: UserId::from_str("5b4c96ac-316a-4bee-8e69-cac5eb84ff4c")?,
        })
        .await?;
        let book = repo.find_by_id(book_id).await?;

        assert!(book.is_none());

        Ok(())
    }

    #[sqlx::test(fixtures("common", "book_list"))]
    async fn test_list_filters(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let repo = BookRepositoryImpl::new(ConnectionPool::new(pool.clone()));

        const LEN: i64 = 50; // 50 is the number of records of fixtures "book_list"

        let res = repo
            .find_all(BookListOptions {
                limit: 10,
                offset: 0,
            })
            .await?;
        assert_eq!(res.total, LEN);
        assert_eq!(res.limit, 10);
        assert_eq!(res.offset, 0);
        assert_eq!(res.items[0].title, "title050");

        let res = repo
            .find_all(BookListOptions {
                limit: 10,
                offset: 10,
            })
            .await?;
        assert_eq!(res.total, LEN);
        assert_eq!(res.limit, 10);
        assert_eq!(res.offset, 10);
        assert_eq!(res.items[0].title, "title040");

        let res = repo
            .find_all(BookListOptions {
                limit: 10,
                offset: 100,
            })
            .await?;
        assert_eq!(res.total, 0); // offsetがtotalを超える場合は0になる
        assert_eq!(res.limit, 10);
        assert_eq!(res.offset, 100);
        assert_eq!(res.items.len(), 0);

        Ok(())
    }
    #[sqlx::test(fixtures("common", "book_checkout"))]
    async fn test_book_checkout(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let book_repo = BookRepositoryImpl::new(ConnectionPool::new(pool.clone()));
        let checkout_repo = CheckoutRepositoryImpl::new(ConnectionPool::new(pool.clone()));

        // 事前登録したユーザーのID（fixtures/book_checkout.sql参照）
        let user_id1 = UserId::from_str("9582f9de-0fd1-4892-b20c-70139a7eb95b").unwrap();
        let user_id2 = UserId::from_str("050afe56-c3da-4448-8e4d-6f44007d2ca5").unwrap();

        let book = book_repo
            .find_all(BookListOptions {
                limit: 20,
                offset: 0,
            })
            .await?
            .into_inner()
            .pop()
            .unwrap();

        // 初期の貸し出し状態がNoneであることを確認
        assert!(book.checkout.is_none());

        // 1回目の貸し出し（user_id1）の蔵書の戻り値のテスト
        {
            checkout_repo
                .create(CreateCheckout {
                    book_id: book.id,
                    checked_out_by: user_id1,
                    checked_out_at: Utc::now(),
                })
                .await?;

            // 貸し出しがある状態での蔵書の戻り値
            // -> Book#checkoutが存在し、貸し出し時に指定したユーザーIDになっている
            let book_co = book_repo.find_by_id(book.id).await?.unwrap();
            assert!(book_co.checkout.is_some());
            let co = book_co.checkout.unwrap();
            assert_eq!(co.checked_out_by.id, user_id1);

            // 返却を実行
            checkout_repo
                .update_returned(UpdateReturned {
                    checkout_id: co.checkout_id,
                    book_id: book_co.id,
                    returned_by: user_id1,
                    returned_at: Utc::now(),
                })
                .await?;

            // 返却後の蔵書の戻り値
            // -> Book#checkoutが存在し、貸し出し時に指定したユーザーIDになっている
            let book_re = book_repo.find_by_id(book.id).await?.unwrap();
            assert!(book_re.checkout.is_none());
        }

        // 2回目の貸し出し（user_id2）の蔵書の戻り値のテスト
        // 2回目の貸出中にuser_id1の情報が返されないことを確認する
        {
            checkout_repo
                .create(CreateCheckout {
                    book_id: book.id,
                    checked_out_by: user_id2,
                    checked_out_at: Utc::now(),
                })
                .await?;

            // 貸し出しがある状態での蔵書の戻り値
            // -> Book#checkoutが存在し、貸し出し時に指定したユーザーIDになっている
            let book_co = book_repo.find_by_id(book.id).await?.unwrap();
            assert!(book_co.checkout.is_some());
            let co = book_co.checkout.unwrap();
            assert_eq!(co.checked_out_by.id, user_id2);

            // 返却を実行
            checkout_repo
                .update_returned(UpdateReturned {
                    checkout_id: co.checkout_id,
                    book_id: book_co.id,
                    returned_by: user_id2,
                    returned_at: Utc::now(),
                })
                .await?;

            // 返却後の蔵書の戻り値
            // -> Book#checkoutが存在し、貸し出し時に指定したユーザーIDになっている
            let book_re = book_repo.find_by_id(book.id).await?.unwrap();
            assert!(book_re.checkout.is_none());
        }

        Ok(())
    }
}
