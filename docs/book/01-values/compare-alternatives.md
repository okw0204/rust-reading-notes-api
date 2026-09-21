# 成立する実装と別案を比べる

## 問い

本編の登録処理は、なぜ入力を値で受け取り、repository にだけ参照を渡し、`Clone` を挟まず、`?` で失敗を返すのでしょうか。成立する別案と、意図した所有権・借用の規則で拒否される例を比べます。

この章の短い例は比較のためのコードです。本編で使う完成形は、ソースから取り込む次の実装です。

```rust,ignore
{{#include ../../../src/service.rs:create_book_service}}
```

## 部分的な移動

### 成立する例

本編の handler は、一つの入力 DTO から 2 つのフィールドを順に移します。小さくすると次の形です。

```rust
struct Request {
    title: String,
    author: String,
}

let request = Request {
    title: "Rust Book".to_owned(),
    author: "The Rust Project".to_owned(),
};

let title = request.title;
let author = request.author;
```

`title` を移した後も、まだ移していない `author` は使えます。フィールドを個別に取り出す部分的な移動が成立するためです。

### 意図的にコンパイルできない例

一部を移した後に構造体全体を使うと、部分的に移動済みの値を借りることになるため拒否されます。

```rust,compile_fail,E0382
#[derive(Debug)]
struct Request {
    title: String,
    author: String,
}

let request = Request {
    title: "Rust Book".to_owned(),
    author: "The Rust Project".to_owned(),
};

let title = request.title;
println!("{request:?}");
```

拒否される理由は `request.title` の所有権が `title` へ移ったことです。`Debug` の不足やフィールドの可視性ではありません。残っている `request.author` だけを使う例とは区別します。

構造体からすべての値を取り出すことを明示したい場合は、分解も使えます。

```rust
let Request { title, author } = request;
```

これは本編と所有権の結果が同じ、成立する書き方です。フィールドが 2 つだけでそのまま別の構造体を作る本編では、`request.title` と `request.author` の対応が直接見える現在の形を使っています。

## 借用してから所有する値を作る

`normalize` は `String` を受け取り、`trim` で内容を借用してから、結果を所有する `String` を作ります。

```text
String を受け取る
  → trim で &str を借りる
  → 空なら Err
  → 必要な範囲を新しい String にして Ok
```

引数を `&str` にする別案も成立します。

```rust
fn normalize(value: &str) -> Result<String, InvalidText> {
    let value = value.trim();
    if value.is_empty() {
        return Err(InvalidText("title must not be empty"));
    }
    Ok(value.to_owned())
}
```

ただし、`BookTitle` が検証後の文字列を所有する以上、成功時の割り当ては必要です。現在の `TryFrom<String>` は HTTP 入力の `String` を以後使わないことを型シグネチャで示し、変換へ所有権を渡します。借用版は、呼び出し側が元の文字列を引き続き必要とする Interface なら適します。本編ではその必要がありません。

### 意図的にコンパイルできない例

借用した範囲を後で使う間に、元の文字列を変更することはできません。

```rust,compile_fail,E0502
let mut title = "  Rust Book  ".to_owned();
let trimmed = title.trim();
title.clear();
println!("{trimmed}");
```

`trimmed` が `title` の内容を指しているのに `clear` が内容を変更すると、参照が有効なままではいられません。拒否理由は、同じ期間の不変借用と可変借用の衝突です。`trimmed` を最後に使った後なら `title.clear()` を実行できます。

## `Clone` を挟む別案

次の書き方はコンパイルできますが、本編には採用していません。

```rust
let title = BookTitle::try_from(input.title.clone())?;
let author = Author::try_from(input.author.clone())?;
```

`clone` すると入力 DTO と値型の変換用に同じ文字列を 2 つ持てます。しかし、その後で `input` を使わない本編では、元の 2 つの文字列を残す意味がありません。複製の割り当てとコピーだけが増えます。

所有権エラーを消すために無条件で `Clone` を足すのではなく、次を確認します。

1. 元の値を後で本当に使うか。
2. 一時的に読むだけなら `&T` を渡せるか。
3. 呼び出し先へ役割を引き渡すなら、値を移せるか。

本編では、HTTP 入力から値型までは役割が移るため `String` を move し、保存処理は値を読むだけなので `&BookTitle` と `&Author` を borrow します。

## `?` を `match` で展開する別案

本編の 1 行は、概念的には次の `match` と同じ制御です。

```rust
let title = match BookTitle::try_from(input.title) {
    Ok(title) => title,
    Err(error) => return Err(AppError::from(error)),
};
```

この別案も成立します。`?` は成功時の値を取り出し、失敗時には `From` で呼び出し元のエラー型へ変換して早期に戻る、という同じ処理を短く表します。

`?` は後続処理を実行してからエラーを記録する仕組みではありません。書名の変換に失敗すれば、著者の変換にも repository にも到達しません。HTTP 応答への変換はさらに外側の Axum の境界で行われます。

## 比較のまとめ

| 選択 | 成立するか | 本編での判断 |
| --- | --- | --- |
| フィールドを順に move | 成立する | 入力を使い切るため採用 |
| DTO を分解して move | 成立する | 同じ意味だが現在の形の方が対応を追いやすい |
| 一部を move 後に DTO 全体を使う | 成立しない | 部分的に移動済みのため拒否 |
| 変換へ `&str` を渡す | 成立する | 元の入力を残す要件がないため不採用 |
| 入力文字列を `clone` して変換する | 成立する | 不要な複製になるため不採用 |
| 不変参照の使用中に元の文字列を変更する | 成立しない | 借用が競合するため拒否 |
| `?` を `match` へ展開する | 成立する | 同じ失敗伝播を簡潔に表すため `?` を採用 |

## 確認

1. `request.title` を移した後に `request.author` は使えても、`request` 全体を使えないのはなぜですか。
2. `TryFrom<String>` を `TryFrom<&str>` に変えれば、成功時の文字列確保もなくなりますか。
3. 現在の登録処理で `input.title.clone()` が不要なのはなぜですか。
4. `?` を `match` に展開したとき、`InvalidText` はどこで `AppError` へ変わりますか。

## 解答

1. `title` だけが別の所有者へ移り、`author` はまだ残っているからです。ただし一部を失った `request` は完全な構造体ではありません。
2. なくなりません。`BookTitle` が入力とは独立して文字列を所有するため、借用した `&str` から所有する `String` を作る必要があります。
3. 入力 DTO を後で使わず、値型へ所有権を引き渡せるからです。clone しても観測できる振る舞いは増えず、割り当てとコピーだけが増えます。
4. `Err(error) => return Err(AppError::from(error))` の `AppError::from` です。`?` はこの変換と早期 return を短く書いています。

第 1 部では、本の登録を通して値の移動、借用、失敗の伝播を追いました。次は[検証済みの文字列を値型にする](../02-types/validated-values.md)で、構築後の型が何を保証し、何を保証しないかを詳しく読みます。
