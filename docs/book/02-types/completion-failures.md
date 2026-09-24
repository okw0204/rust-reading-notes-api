# 失敗後に何が残るか

## 問い

正しい一括読了記録の中に、存在しない本、読書中ではない本、SQLite の失敗が含まれたとき、なぜ要求全体を HTTP エラーにせず一冊ごとの読了結果を返すのでしょうか。一冊の失敗後に保存状態がどうなるかを、冊子間の部分成功と一冊内の原子性に分けて追います。

## 前提

- [要求から検証済みの入力へ](../01-values/reading-completion-input.md)で扱う、保存前の要求全体の検証。
- [保存状態を型状態へ接続する](stored-state-to-typestate.md)で扱う `Book<Reading>` から `Book<Finished>` への遷移。
- [状態変更とメモを一緒に保存する](atomic-completion.md)で扱う、保存時の状態検査と transaction。

## 読む場所と順序

1. `src/service.rs` の `ReadingCompletionResult`、`ReadingCompletionFailure`、`From<AppError>`。
2. 同ファイルの `ReadingService::record_reading_completions` と、一冊を処理する `record_reading_completion`。
3. `src/handler.rs` の `ReadingCompletionResultResponse` と `From<ReadingCompletionResult>`。
4. `src/repository.rs` の `record_reading_completion` の契約。
5. `src/repository/sqlite.rs` の `record_reading_completion`。
6. `tests/api.rs` の `returns_item_failures_without_rolling_back_independent_successes`、`rolls_back_a_completion_when_adding_its_note_fails`、`rejects_the_entire_completion_request_before_saving_any_item`。

## 要求全体の失敗と一冊の失敗を分ける

```rust,ignore
{{#include ../../../src/service.rs:reading_completion_failures}}
```

`record_reading_completions` の返り値は `Result<Vec<ReadingCompletionResult>, AppError>` です。外側の `Result` と内側の `ReadingCompletionResult` は、失敗する範囲が異なります。

| 失敗 | 表現 | HTTP | 保存結果 |
| --- | --- | --- | --- |
| 件数、重複、空のメモ本文 | 外側の `Err(AppError::Validation)` | `400 Bad Request` | 全項目を処理前に拒否し、どの本も変更しない |
| 本が存在しない | `Failed` と `NotFound` | 要求は `200 OK`、項目は `not_found` | その本について変更なし |
| 本が読書中ではない、または保存時に状態が変わった | `Failed` と `Conflict` | 要求は `200 OK`、項目は `conflict` | その本について変更なし |
| DB の操作や保存値の復元に失敗 | `Failed` と `Internal` | 要求は `200 OK`、項目は `internal_error` | transaction が確定していなければ変更なし |

入力規則の違反は、処理してよい一括読了記録がまだ成立していないため、要求全体を拒否します。入力全体が正しければ、一冊ごとの読了記録は互いに独立しています。ある本の未検出や競合を、別の本まで失敗させる理由にはしません。

`From<AppError>` は repository から返った内部の失敗を、読了結果で公開する 3 種類へ変換します。`Database` と `InvalidStoredValue` は原因をログへ残しますが、利用者向けにはどちらも `Internal` です。SQL、trigger、保存されていた不正な値などの詳細は応答へ含めません。repository から想定外の `Validation` が返った場合も、利用者の入力不正とは扱わず内部失敗として記録します。

## 一冊の失敗を結果として扱う

`ReadingCompletionResult::Failed` は、一括読了記録そのものを失敗させる `AppError` ではなく、一冊分の読了結果です。service は一冊の失敗をこの値へ変換し、ほかの本の結果とともに返します。各処理の進み方や完了順にかかわらず、利用者へ返す `results` は入力との対応を保つことが契約です。

handler は `ReadingCompletionResult` を次のどちらかへ変換します。

```json
{"book_id":1,"outcome":"completed","book":{"id":1,"title":"Book","author":"Author","status":"finished"},"note":{"id":1,"body":"読了メモ"}}
```

```json
{"book_id":2,"outcome":"failed","error":{"code":"conflict","message":"reading state conflict"}}
```

正しい入力を受理した一括読了記録は、一部または全部の項目が失敗しても `200 OK` です。HTTP status だけで全冊成功と判断せず、各 `outcome` を読みます。

## 一冊の中では片方だけを残さない

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:reading_completion_transaction}}
```

一冊の読了記録は、状態を `finished` へ更新し、メモを追加してから transaction を commit します。更新対象がなければ `Conflict` です。メモ追加や commit が失敗して関数を途中で抜けると transaction は rollback され、状態変更だけを残しません。

ここには 2 つの保存範囲があります。

- 一冊の中: 状態変更とメモ追加は、両方が残るか、どちらも残らない。
- 複数冊の間: 一冊の失敗は、別の本ですでに確定した読了記録を取り消さない。

全冊を一つの transaction に入れると、一冊の競合で独立した成功まで失います。反対に、状態更新とメモ追加を別々に commit すると、読了だけが残る可能性があります。現在の実装は、一冊の原子性と冊子間の部分成功を別の境界で表します。

## テストから保存状態を読む

`returns_item_failures_without_rolling_back_independent_successes` は、存在しない本、未読の本、読書中の本を 1 つの要求に入れます。応答が入力順に `not_found`、`conflict`、`completed` となることに加え、成功した本だけが読了とメモを保存し、未読の本は状態もメモも変わらないことを `GET /books/{id}` で確認します。

`rolls_back_a_completion_when_adding_its_note_fails` は、`notes` への `INSERT` を必ず拒否する SQLite trigger をテスト内で作ります。実時間や偶然の実行順ではなく、状態更新の後にメモ保存だけを決定的に失敗させます。応答は原因を隠した `internal_error` で、再取得した本は読書中のまま、メモは空です。これにより、HTTP のエラー表現と transaction の rollback を同じ経路から観測します。

`rejects_the_entire_completion_request_before_saving_any_item` は、正しい先頭項目と空の後続項目を送ります。この場合は一冊の失敗ではなく入力全体の失敗なので `400 Bad Request` となり、先頭項目も保存されません。

これらの有限なテストが示すのは、実行した入力、競合、注入した DB 失敗で観測した契約です。将来のすべての SQLite 障害や任意の実行順を網羅するわけではありません。

## 別案との比較

### 最初の一冊の失敗を要求全体の HTTP エラーにする

実装は `?` で短く書けますが、どの入力まで保存されたかを応答から対応付けられず、独立した後続項目も処理されません。入力が正しい要求では一冊ごとの結果を返す契約に合わないため採用しません。

### 全冊を一つの transaction にする

全体成功・全体失敗が必要な処理なら成立します。一括読了記録では本ごとの状態とメモが独立しており、部分成功を利用者へ返すため採用しません。

### 状態更新とメモ追加を別々に保存する

transaction の範囲は小さく見えますが、後半の失敗で読了状態だけが残ります。「読了記録は状態変更とメモが一緒に成立する」という契約を破るため採用しません。

## 確認

1. `Result<Vec<ReadingCompletionResult>, AppError>` の外側と内側は、どの失敗を表しますか。
2. 一冊が `not_found` でも、後続の本を処理するのはなぜですか。
3. `Book<Finished>` を作ったあとも transaction が必要なのはなぜですか。
4. DB の詳細を `internal_error` の本文へ含めない一方、原因をログへ残すのはなぜですか。
5. trigger を使うテストは何を決定的にし、何までは保証しませんか。

## 解答

1. 外側は件数、重複、メモ本文という要求全体の入力規則を表し、失敗時は保存前に `400` を返します。内側は入力が正しい要求に含まれる一冊の成功・失敗を表し、`200` の `results` に並びます。
2. 冊子間の読了記録は独立し、一冊の未検出を理由に別の本の成功を失わせない契約だからです。失敗を値として `results` に追加し、外側へ早期 return しません。
3. 型状態は手元の値が合法に遷移したことを示しますが、DB の現在状態と、状態変更・メモ追加の同時確定は保証しません。条件付き更新と transaction が実行時の保存契約を担います。
4. 利用者は再試行や状態確認に必要な分類だけを知ればよく、DB の構造や保存値を公開する必要はありません。一方、運用側は原因を調べる必要があるため、内部ログには元の失敗を残します。
5. 状態更新の後のメモ追加が必ず失敗する経路を、待ち時間や実行順に依存せず作ります。その経路で rollback されることは示しますが、すべての DB 障害や commit 失敗を網羅しません。

次は[`BookRepository` の契約](../03-abstraction/repository-trait.md)で、一冊の原子的な保存を SQLite とフェイクが同じ Interface の背後でどう表すかを読みます。
