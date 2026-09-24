# 要求から検証済みの入力へ

## 問い

`POST /reading-completions` が複数冊の `book_id` とメモ本文を受け取ったとき、なぜ一件ずつ保存しながら検証せず、要求全体を先に検証するのでしょうか。HTTP の `String` が検証済みの `NoteBody` へ変わるまでの所有権も追います。

## 前提

- `Vec<T>` が要素を所有すること。
- `Result` と `?` による早期 return。
- 前後の空白を除いても空でない本文だけを表す `NoteBody` の役割。

## 読む場所と順序

1. `src/app.rs` の `/reading-completions` route。
2. `src/handler.rs` の `ReadingCompletionsRequest` と `ReadingCompletionItemRequest`。
3. `src/handler.rs` の `record_reading_completions`。
4. `src/service.rs` の `ReadingService::record_reading_completions`。まず保存を始める前の検証ブロックを読む。
5. `src/repository.rs` の `BookRepository::record_reading_completion`。
6. `src/repository/sqlite.rs` の同名メソッド。
7. `tests/api.rs` の `records_reading_completions_and_returns_the_persisted_results` と `rejects_the_entire_completion_request_before_saving_any_item`。

## HTTP の所有値

```rust,ignore
{{#include ../../../src/handler.rs:reading_completions_request}}
```

Axum の `Json<ReadingCompletionsRequest>` が JSON を受理すると、handler は `items: Vec<ReadingCompletionItemRequest>` を所有します。各要素も `body: String` を所有します。handler は `into_iter()` で `Vec` を消費し、`book_id` と `body` を service の入力へ移します。入力をあとで使い直さないため、ここで `String` や一覧を clone する必要はありません。

JSON の構造が壊れている、必須フィールドがない、`book_id` が数値でない、といった失敗は `Json` extractor が handler 本文より前に拒否します。service が検査するのは、構造としては受理できた要求に対するアプリケーションの規則です。

## 保存前に要求全体を検証する

```rust,ignore
{{#include ../../../src/service.rs:reading_completions_validation}}
```

検証は次の順で進みます。

1. `items` の件数が 1 件以上 8 件以下か調べる。
2. `HashSet` に既出の `book_id` を入れ、同じ要求内の重複を拒否する。
3. 各 `body: String` を `NoteBody::try_from` へ移し、前後の空白を除いても空なら拒否する。
4. すべて成功した項目だけを `validated` に入れる。

`validated` を作り終えるまで repository は呼ばれません。先頭の項目を保存してから後続の空本文を見つける形では、`400 Bad Request` を返した要求の一部だけが残ってしまいます。要求全体の入力規則は、保存処理を一つでも始める前に確定させます。

`NoteBody::try_from(item.body)` は `String` を値で受け取ります。成功時は `NoteBody` が正規化済みの本文を所有し、失敗時は `?` が `AppError::Validation` を返します。`item.body` を借りたまま保持するのではないため、元の HTTP DTO の寿命から切り離して後続の Future へ渡せます。

## 検証後に保存結果まで通す

検証済みの各項目について、service は本を取得し、保存された読書状態が読書中であることを確認します。`StoredBook::Reading(book)` から取り出した `Book<Reading>` を `finish()` が消費し、`Book<Finished>` を作ります。repository はその本と `&NoteBody` を受け取ります。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:reading_completion_transaction}}
```

SQLite Adapter は一冊について、現在状態が `reading` の行だけを `finished` へ更新し、同じ transaction でメモを追加します。commit 後に返した本とメモが HTTP 応答の `book` と `note` になります。handler は結果を `outcome: "completed"` とともに JSON へ変換します。

この章で重要なのは、型による保証だけで保存を済ませていない点です。`Book<Finished>` は手元の値が合法な遷移で作られたことを示しますが、DB の現在状態やメモとの同時保存までは保証しません。そのため Adapter は `WHERE status = 'reading'` と transaction を使います。

## 正常系と拒否をどこから観測するか

`tests/api.rs` の正常系は Router と実 SQLite を使い、2 冊を読書中にしてから一括読了記録を要求します。応答に読了済みの本とメモが入力順で含まれることに加え、`GET /books/{id}` で同じ状態とメモを再取得します。応答 DTO を見ただけではなく、利用者が使う Interface から保存結果まで一致することを観測しています。

拒否のテストは、先頭に正しい項目、後続に空白だけの本文を置きます。`400 Bad Request` のあとで両方の本を再取得し、どちらも読書中のままでメモがないことを確認します。これにより「不正な項目自身を保存しない」だけでなく、「要求全体を保存前に検証する」という規則を観測できます。

## 別案との比較

### 各項目を処理する直前に検証する

一件だけの要求なら同じ結果になります。しかし複数件では、後続の入力不正を見つける前に先行項目を保存し得ます。入力規則の失敗では要求全体を拒否する契約と合わないため採用しません。

### HTTP DTO の `String` を最後まで使う

型の数は減りますが、どこから空でない本文を前提にしてよいか分からなくなります。検証後だけ作れる `NoteBody` へ所有権を移すことで、repository は空本文の検査を繰り返さずに済みます。

### 要求全体を clone して検証用と保存用に分ける

実装は書けますが、検証が終わった元の `String` と clone 後の値が同じ規則を満たすことを型で区別できず、割り当ても増えます。入力を一度だけ消費し、検証済みの値へ変換する方が所有者と保証を追いやすくなります。

## 確認

1. JSON の `body` は、handler から service、`NoteBody` へどのように渡りますか。
2. 正しい先頭項目が、後続の空本文より先に保存されないのはなぜですか。
3. `Book<Finished>` が作れたあとも、SQLite の更新条件が必要なのはなぜですか。
4. 正常応答だけでなく `GET /books/{id}` でも確認することで、何が分かりますか。

## 解答

1. `ReadingCompletionItemRequest` が `String` を所有し、handler の `into_iter()` が service の入力へ移します。service はさらに `NoteBody::try_from` へ移し、成功した `NoteBody` が正規化済みの本文を所有します。clone は行いません。
2. service が全項目を走査して `validated` を完成させたあとにだけ repository を呼ぶためです。途中で `?` が失敗すると保存ループへ到達しません。
3. 型状態が保証するのは手元の値の遷移です。取得後に DB の状態が変わる競合と、状態変更とメモ追加を一緒に確定することは、実行時の条件付き更新と transaction が保証します。
4. 応答として値を組み立てられたことだけでなく、その本の読書状態とメモが SQLite に残り、利用者向けの再取得経路から同じ結果を読めることが分かります。

次は[借用して検査し、所有して渡す](borrow-and-own.md)で、検査中の借用から子 Future が持つ所有値へ切り替える理由を読みます。
