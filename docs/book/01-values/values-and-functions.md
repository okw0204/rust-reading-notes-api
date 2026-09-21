# 値を受け取る関数を読む

## 問い

HTTP から届いた書名の `String` は、どの関数へ所有権を渡し、どこで一時的に借用され、どのような `Result` になって戻るのでしょうか。

この章ではアプリケーション全体からいったん離れ、`BookTitle::try_from` と `normalize` だけを読みます。小さな関数で値の出入りを説明できてから、本の登録経路へ範囲を広げます。

## 読む場所と順序

1. `src/domain/text.rs` の `BookTitle` と `InvalidText`。
2. 同じファイルの `normalize`。
3. `TryFrom<String> for BookTitle` と `as_str`、`into_inner`。
4. `src/service.rs` の `ReadingService::create_book`。
5. `src/error.rs` の `From<InvalidText> for AppError`。

```rust,ignore
{{#include ../../../src/domain/text.rs:validated_title}}
```

## 解説

### 引数の型から所有権を読む

`normalize` の引数は `value: String` です。`&str` ではないため、呼び出し側は `String` の所有権をこの関数へ移します。関数内の `value` が新しい所有者です。

```text
入力の String
  └─ move → normalize の value
                  ├─ borrow → trim が返す &str
                  └─ allocate → 成功時に新しい String
```

`value.trim()` は元の文字列を変更しません。`trim` は `value` を借用し、前後の空白を除いた範囲を指す `&str` を返します。変数名を同じ `value` にしているため、以降の `value` はこの参照です。元の `String` は関数が所有したままで、参照が使われる間は破棄されません。

空白を除いた結果が空なら、`Err(InvalidText(message))` を返します。文字が残っていれば `to_owned()` で、その範囲を所有する新しい `String` を 1 つ作ります。元の入力を移動したことは、割り当てが一度も起きないことを意味しません。返す `String` は入力が破棄された後も独立して保持する必要があるためです。

### `TryFrom` が成功と失敗を型にする

`BookTitle::try_from` も `String` を値で受け取ります。成功時は `Ok(BookTitle)`、空白だけなら `Err(InvalidText)` です。`normalize(...).map(Self)` は、成功した `String` だけを `BookTitle` で包み、エラーはそのまま残します。

`BookTitle` の内部フィールドは非公開です。ここでは「検証済みの値を作る入口が `TryFrom` に限られる」ことだけ押さえます。可視性と不変条件の範囲は第 2 部で詳しく読みます。

`as_str(&self) -> &str` は `BookTitle` を借り、内部の文字列への参照を返します。`into_inner(self) -> String` は `BookTitle` 自体を受け取り、内部の `String` を呼び出し側へ移します。同じ値型でも、メソッドの `&self` と `self` から借用か移動かを区別できます。

### `?` は失敗なら早く戻る

本の登録では次の実装が値型を作ります。

```rust,ignore
{{#include ../../../src/service.rs:create_book_service}}
```

`input.title` は `BookTitle::try_from` へ移ります。成功すれば `title` が得られ、失敗すれば最初の `?` で `create_book` から早期に戻ります。このとき `InvalidText` は `From<InvalidText> for AppError` により `AppError::Validation` へ変換されます。

最初の変換に失敗した場合、`input.author` の変換や repository の呼び出しは行われません。`?` はログを出す命令でも HTTP 応答を作る命令でもなく、現在の関数の `Result` に合わせてエラーを返す記法です。

## 確認

1. `normalize` が `String` を受け取った後、`trim` は文字列の所有権をさらに受け取りますか。
2. 成功時に `to_owned()` が必要なのはなぜですか。
3. `BookTitle::try_from(input.title)?` が失敗したとき、著者の変換と保存は実行されますか。
4. `as_str` と `into_inner` は、`BookTitle` の所有権をどう扱い分けますか。

## 解答

1. 受け取りません。`trim` は `String` の内容を `&str` として借用します。所有者は `normalize` 内の元の `String` のままです。
2. `trim` が返す値は入力を指す参照だからです。関数の外へ所有する値を返すため、必要な範囲から新しい `String` を作ります。
3. 実行されません。最初の `?` が変換エラーを `AppError` へ変えて早期に返します。
4. `as_str(&self)` は値型を借りて `&str` を返します。`into_inner(self)` は値型を消費し、内部の `String` を呼び出し側へ移します。

次は[本の登録で所有権を追う](create-book.md)で、この小さな変換の前後に HTTP と SQLite の境界をつなぎます。
