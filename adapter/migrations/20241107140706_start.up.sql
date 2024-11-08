-- Add up migration script here
CREATE
OR REPLACE FUNCTION set_updated_at () RETURNS TRIGGER AS '
  BEGIN
    new.updated_at := ''now'';
    return new;
  END;
' LANGUAGE 'plpgsql';

-- books テーブルに蔵書の所有者を表す user_id を追記する
CREATE TABLE
  IF NOT EXISTS books (
    book_id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
    title VARCHAR(255) NOT NULL,
    author VARCHAR(255) NOT NULL,
    isbn VARCHAR(255) NOT NULL,
    description VARCHAR(1024) NOT NULL,
    user_id UUID NOT NULL, -- この行を追加
    created_at TIMESTAMP(3)
    WITH
      TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
      updated_at TIMESTAMP(3)
    WITH
      TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP(3)
  );

CREATE TRIGGER books_updated_at_trigger BEFORE
UPDATE ON books FOR EACH ROW EXECUTE PROCEDURE set_updated_at ();