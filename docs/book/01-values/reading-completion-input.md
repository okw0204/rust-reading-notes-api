# 要求から検証済みの入力へ

## 問い

`POST /reading-completions` が複数冊の `book_id` とメモ本文を受け取ったとき、なぜ一件ずつ保存しながら検証せず、要求全体を先に検証するのでしょうか。HTTP の `String` が検証済みの `NoteBody` へ変わるまでを、実コードの順に追います。

## 前提

- `Vec<T>` が要素を所有すること。
- `Result` と `?` による早期 return。
- 前後の空白を除いても空でない本文だけを表す `NoteBody` の役割。

## 読む場所と順序

1. Router の登録: `src/app.rs` の `/reading-completions` route。
2. HTTP DTO と handler の入力変換: `src/handler.rs` の `ReadingCompletionsRequest` と `record_reading_completions`（[HTTP の所有値](#http-の所有値)、[handler の入力変換](#handler-の入力変換)）。
3. 保存前の全件検証: `src/service.rs` の `record_reading_completions`（[保存前の全件検証](#保存前の全件検証)）。
4. 本文の型と正規化: `src/domain/text.rs` の `NoteBody`、`normalize`、`TryFrom<String>`（[本文の正規化と NoteBody](#本文の正規化と-notebody)）。
5. 検証後に呼ばれる保存経路: repository と transaction は[保存状態を型状態へ接続する](../02-types/stored-state-to-typestate.md)と[状態変更とメモを一緒に保存する](../02-types/atomic-completion.md)で読みます。

## HTTP の所有値

対象: `src/handler.rs` / `ReadingCompletionsRequest` と `ReadingCompletionItemRequest`

```rust,ignore
{{#include ../../../src/handler.rs:reading_completions_request}}
```

- `ReadingCompletionsRequest` は一つのフィールド `items: Vec<ReadingCompletionItemRequest>` を持ちます。Axum の `Json<ReadingCompletionsRequest>` が JSON を受理すると、handler はこの `Vec` を所有します。
- `ReadingCompletionItemRequest` は `book_id: BookId` と `body: String` を持ちます。`Vec` が各要素を所有し、各要素が自分の `String` を所有するため、項目数ぶんの本文が handler の手元にそろいます。
- `book_id` が数値でない、必須フィールドがない、JSON の構造が壊れている、といった失敗は `Json` extractor が handler 本文より前に拒否します。service が検査するのは、構造としては受理できた要求に対するアプリケーションの規則です。

## handler の入力変換

対象: `src/handler.rs` / `record_reading_completions`（入力を service の入力へ移す部分）

```rust,ignore
{{#include ../../../src/handler.rs:completion_handler_input}}
```

- `request.items.into_iter()` は `Vec` を値で消費し、各 `ReadingCompletionItemRequest` を所有したまま順に取り出します。借用して `&ReadingCompletionItemRequest` を返す `iter()` と違い、要素そのものを取り出せるので、`book_id` と `body` を service の入力へ移せます。
- `.map(|item| RecordReadingCompletion { book_id: item.book_id, body: item.body })` の `|item|` は各要素を受け取るクロージャです。ここで HTTP の DTO から service の入力型へ詰め替えます。`BookId` は `Copy` なので `book_id` はコピーされ、`body: String` は所有権が移動します。`body` を clone しないのは、元の DTO をあとで使わないためです。
- `.collect()` はイテレータを実際に走査し、`RecordReadingCompletions.items` のフィールド型から `Vec<RecordReadingCompletion>` を推論して一覧を組み立てます。値の移動は `.collect()` が走り切った時点で完了します。
- できあがった `RecordReadingCompletions` を `state.service.record_reading_completions(...)` へ値で渡し、`.await?` でその Future の完了を待ちます。この後 handler は元の `request` を使いません。
- `body` を移した後の `item` 全体は使えませんが、両方のフィールドを取り切るこの変換では、使わない値を残す理由がありません。部分的な移動の意味は[借用して検査し、所有して渡す](borrow-and-own.md)で詳しく読みます。

## 保存前の全件検証

対象: `src/service.rs` / `ReadingService::record_reading_completions`（保存前の全件検証）

```rust,ignore
{{#include ../../../src/service.rs:reading_completions_validation}}
```

- `!(1..=8).contains(&input.items.len())` は、一括読了記録の件数を 1 件以上 8 件以下に制限します。範囲外は `AppError::Validation` を返します。
- `HashSet::with_capacity` と `Vec::with_capacity` は、件数ぶんの領域を先に確保します。件数はすでに 8 以下と分かっているので、追加時の再確保を避けられます。
- `for item in input.items` は `Vec` を消費して各要素を値で取り出します。`seen_book_ids.insert(item.book_id.0)` は `BookId` の内部値で重複を調べ、すでにあれば `false` を返すので、同じ要求内の重複を拒否します。
- 重複がなければ、`NoteBody::try_from(item.body)?` が本文の `String` を `normalize` へ移して検証します。成功時は、trim 後の範囲から新しく確保した `String` を所有する `NoteBody` と `book_id` の組を `validated` に積みます。`?` が `InvalidText` を `AppError::Validation` へ変え、失敗時はここで早期 return します。

`validated` が完成するまで repository は一度も呼ばれません。先頭項目を保存したあとで後続の空本文を見つけると、`400 Bad Request` を返した要求の一部だけが保存されます。これでは応答と保存状態が一致しません。

要求全体の入力規則は、保存処理を一つでも始める前に確定させます。

入力規則の違反が `AppError::Validation` から `400 validation_error` になる対応は、[失敗後に何が残るか](../02-types/completion-failures.md)の表で確認できます。

## 本文の正規化と NoteBody

対象: `src/domain/text.rs` / `normalize`

```rust,ignore
{{#include ../../../src/domain/text.rs:normalize_text}}
```

- 引数 `value: String` は本文を値で受け取り、normalize がその所有権を持ちます。
- `value.trim()` は前後の空白を除いた範囲を指す `&str` を返し、元の `String` を借用するだけです。ここでは新しい文字列を確保しません。
- 空なら `&'static str` のメッセージを持つ `InvalidText` を返します。この分岐では新しい文字列を確保せず、引数として受け取った元の `String` は `normalize` を抜けるときに破棄されます。
- 空でなければ `value.to_owned()` で trim 後の範囲から新しい `String` を一度だけ作り、その所有権を返します。trim の借用をそのまま返すと元の `String` の寿命に縛られるため、独立して生存できる所有値へ切り替えます。

対象: `src/domain/text.rs` / `TryFrom<String> for NoteBody` と `NoteBody`

```rust,ignore
{{#include ../../../src/domain/text.rs:note_body_conversion}}
```

- `TryFrom<String>` は `String` を値で受け取り、`normalize(value, "note body must not be empty")` の結果へ `map(Self)` します。成功時は `NoteBody` が正規化済みの `String` を所有し、失敗時は `InvalidText` です。
- service の `NoteBody::try_from(item.body)?` はこの実装を呼びます。`item.body` の `String` はまず `normalize` へ移り、`trim()` がその中の範囲を借ります。成功時は `to_owned()` が trim 後の範囲から別の `String` を新しく作り、`NoteBody` はその新しい文字列を所有します。元の `String` は `normalize` の終了時に破棄されます。
- `as_str(&self) -> &str` は内部の `String` を借用して貸します。repository が SQL へ bind するときはこの参照を使います。
- `into_inner(self) -> String` は値を消費して内部の `String` を取り出します。応答へ本文を返すときなど、所有権ごと外へ出す場面で使います。

失敗経路も同じ関数から読めます。`"  "` のような空白だけの本文は `normalize` の `is_empty()` で `InvalidText` になり、`?` を通って `400 validation_error` とメッセージ `"note body must not be empty"` になります。

## 検証後に保存結果まで通す

`validated` に入った `(BookId, NoteBody)` は、その後の保存経路で使われます。

- service は一冊ごとに本を取得し、`StoredBook::Reading(book)` の枝で `Book<Reading>` を取り出します。`finish()` がその所有値を消費して `Book<Finished>` を作ります（[保存状態を型状態へ接続する](../02-types/stored-state-to-typestate.md)）。
- repository は `WHERE status = 'reading'` の条件付き更新と transaction で、状態変更とメモ追加を一つの保存単位にまとめます。commit 後の本とメモが応答の `book` と `note` になります（[状態変更とメモを一緒に保存する](../02-types/atomic-completion.md)）。
- `Book<Finished>` が保証するのは手元の値が合法に遷移したことだけです。DB の現在状態と、状態変更・メモ追加を同時に確定することは、実行時の条件と transaction が担います。

## 正常系と拒否をどこから観測するか

正常系のテストは Router と実 SQLite を使い、2 冊を読書中にしてから一括読了記録を要求します。要求本文は入力順が `book_id` 2、1 です。

対象: `tests/api.rs` / `records_reading_completions_and_returns_the_persisted_results`（要求部分）

```rust,ignore
{{#include ../../../tests/api.rs:api_completion_request}}
```

応答は同じ入力順で 2 件とも `outcome: "completed"` になり、`book` と `note` を含みます。さらにテストは `GET /books/{id}` で同じ状態とメモを再取得します。

対象: `tests/api.rs` / `records_reading_completions_and_returns_the_persisted_results`（再取得部分）

```rust,ignore
{{#include ../../../tests/api.rs:api_completion_readback}}
```

応答 DTO を見ただけでなく、利用者が使う Interface から保存結果まで一致することを観測しています。

拒否のテストは、先頭に正しい項目、後続に空白だけの本文を置きます。`400 Bad Request` のあとで両方の本を再取得し、どちらも読書中のままでメモがないことを確認します。

このテストは「不正な項目自身を保存しない」だけでなく、「要求全体を保存前に検証する」規則も観測します。

## 別案との比較

### 各項目を処理する直前に検証する

一件だけの要求なら同じ結果になります。しかし複数件では、後続の入力不正を見つける前に先行項目を保存し得ます。入力規則の失敗では要求全体を拒否する契約と合わないため採用しません。

### HTTP DTO の `String` を最後まで使う

型の数は減りますが、どこから空でない本文を前提にしてよいか分からなくなります。検証後だけ作れる `NoteBody` が正規化後に新しく確保した `String` を所有することで、repository は空本文の検査を繰り返さずに済みます。

### 要求全体を clone して検証用と保存用に分ける

実装は書けますが、検証が終わった元の `String` と clone 後の値が同じ規則を満たすことを型で区別できず、割り当ても増えます。入力を一度だけ消費し、検証済みの値へ変換する方が所有者と保証を追いやすくなります。

## 確認

1. JSON の `body` は、handler から service、`NoteBody` へどのように渡りますか。
2. handler の `into_iter()`、`|item|` のクロージャ、`collect()` は、それぞれ何をしますか。
3. 正しい先頭項目が、後続の空本文より先に保存されないのはなぜですか。
4. `NoteBody::try_from` の失敗は、どの型と HTTP 応答になりますか。
5. 正常応答だけでなく `GET /books/{id}` でも確認することで、何が分かりますか。

## 解答

1. `ReadingCompletionItemRequest` が `String` を所有し、handler の `into_iter()` が各 DTO を service の入力へ移します。service は `body` を `NoteBody::try_from` へ移し、`normalize` が trim 後の範囲から新しい `String` を作ります。成功した `NoteBody` はこの正規化済みの文字列を所有し、元の `String` は破棄されます。
2. `into_iter()` は `Vec` を消費して各要素を取り出します。クロージャは DTO を service の入力型へ詰め替え、`book_id` をコピーして `body` を移動します。最後に `collect()` がイテレータを走査し、一覧を組み立てます。
3. service が全項目を走査して `validated` を完成させたあとにだけ repository を呼ぶためです。途中で `?` が失敗すると保存ループへ到達しません。
4. `normalize` が返す `InvalidText` を `?` が `AppError::Validation` へ変え、`error.rs` の `IntoResponse` が `400 Bad Request` と `validation_error` に変換します。
5. 応答用の値を組み立てられたことに加え、読書状態とメモが SQLite に残ったことも分かります。利用者向けの再取得経路から、同じ結果を読めることまで確認できます。

次は[借用して検査し、所有して渡す](borrow-and-own.md)で、検査中の借用から子 Future が持つ所有値へ切り替える理由を読みます。
