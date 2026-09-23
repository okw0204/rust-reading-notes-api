# Router から HTTP の保証を確かめる

## 問い

HTTP テストで `204 No Content` が返れば、本とメモの削除を確認したと言えるでしょうか。応答の観測と DB の観測を分けて読みます。

## 読む場所と順序

1. `tests/api.rs` の `test_app_and_pool`、`json_body`、`json_request`。
2. 同ファイルの `deletes_a_book_and_its_notes`。
3. 同ファイルの `accepts_only_forward_adjacent_transitions`、`hides_database_error_details`、`rejects_a_non_numeric_book_path`。
4. `src/app.rs` の `build_app`、`src/error.rs` の `IntoResponse`、`migrations/` の外部キー。

```rust,ignore
{{#include ../../../tests/api.rs:api_test_setup}}
```

## 解説

### TCP を開かずに全レイヤーを通す

各テストは `sqlite::memory:` から別の pool を作り、独立したインメモリ SQLite に migration を適用して、本番と同じ `build_app` に渡します。`pool.clone()` は同じ pool へのハンドルなので、テストは Router が使う DB を直接検査できます。SQLx 0.8.6 では URL の解析時に一意な DB 名と `shared_cache = true` を設定し、pool 内で接続オプションを共有するため、同じ pool の複数接続でも同じ DB を共有できます。最大1接続は教材の接続管理を単純にする選択であり、テスト間の DB の独立性や HTTP 処理全体の原子性を保証する設定ではありません。

`tower::ServiceExt` の `oneshot` は Router に一つの `Request<Body>` を渡して応答を待ちます。Router 自体を消費するため、続けて要求を送る場所では `app.clone().oneshot(...)` とします。同じアプリの共有状態を使っており、要求ごとに DB を作り直してはいません。

ここでは TCP の待ち受けや実ネットワーク通信は行いません。一方で routing、extractor、handler、service、SQLite repository、DB、応答への変換は実際に通ります。サーバーのソケット設定などはこの検査範囲に含まれません。

`json_request` は JSON を文字列にして `Body` に入れ、`content-type` を設定します。応答側の `json_body` は `into_body` で本文を取り出し、`collect().await` で全体を集め、バイト列を `serde_json::Value` に変換します。HTTP のステータスと JSON の内容は別々に確認できます。

### 削除を応答と DB の両方から見る

```rust,ignore
{{#include ../../../tests/api.rs:api_delete_test}}
```

本とメモを作った後、削除のステータスが204であることと、本文が空であることを確認します。204の応答に JSON を期待して `json_body` を使うのではなく、生のバイト数を調べています。

続いて `SELECT COUNT(*) FROM notes` を実行し、メモが残っていないことを確かめます。本の削除は repository の `DELETE FROM books`、関連メモの削除は migration の `ON DELETE CASCADE` が担います。成功応答だけではメモが残る不具合を見逃せるため、DB の副作用も観測します。このテストの直接のアサーションは応答とメモ件数であり、本の削除後の GET を追加で送っているわけではありません。

### エラーの種類ごとに通る境界を読む

`accepts_only_forward_adjacent_transitions` は3状態の全9組み合わせを HTTP で試し、前向きの隣接遷移だけ200、それ以外は409と確認します。不許可の後も取得して保存状態を検査するので、エラー応答だけを返して実際は更新してしまう実装も検出できます。

`hides_database_error_details` は `pool.close().await` の後に一覧取得し、500と一般化した `internal_error` の JSON を確認します。[フェイクの保存エラー注入](service-fake.md)と違い、実際の接続 pool の失敗が repository から HTTP まで伝わる検査です。内部の DB エラー詳細は応答に含めません。

`rejects_a_non_numeric_book_path` は、数値に変換できない Path に400が返ることを確認します。これは handler 本文の前に extractor が返す Axum 標準の rejection です。アプリケーションの `AppError` を経由しないので、共通エラー JSON の形までは要求していません。不正な JSON や `Content-Type` の不足も同じく extractor 側の境界として読みますが、このテストがそれらをすべて検査しているわけではありません。

## 確認

1. `oneshot` のテストは TCP を使いますか。どの層を実際に通りますか。
2. 削除のステータスだけでなく、本文と DB を調べる理由は何ですか。
3. `app.clone()` と `pool.clone()` は要求ごとに別の DB を作りますか。
4. 数値でない Path のエラーに、共通の JSON を要求しない理由は何ですか。

## 解答

1. TCP は使いません。Router から extractor、handler、service、SQLite と DB、応答変換まで通ります。
2. 204の空本文という HTTP の契約と、関連メモも消える保存の契約を別々に確かめるためです。
3. いいえ。同じ共有状態・接続 pool を利用します。独立した DB を用意するのはテストの準備段階です。
4. handler 本文より前の extractor の rejection であり、`AppError::into_response` の経路ではないためです。

全体を振り返ると、型状態の doctest は書ける操作、フェイクは service の判断、SQLite テストは保存の契約、Router テストは HTTP を含む入出力を確かめています。[教材の使い方](../introduction.md#5-部の読み方)や画面の目次から気になる章へ戻り、「どの値を、どの境界で、何を根拠に保証するか」を説明してみてください。

## 読了時の総合確認

最後に、教材全体の到達点を一つの処理へ結び直します。次の問いへ、型名や用語だけでなく、値の流れと Interface の契約を根拠に答えてください。

1. `POST /books` で受け取った書名は誰が所有し、どこで移動し、どの保存処理で借用されますか。
2. repository の Future が書名を借りたまま `.await` できるのはなぜですか。その参照は `'static` である必要がありますか。
3. `Book<Reading>` が保証することと、HTTP 入力や SQLite の条件付き更新が実行時に検査することは何ですか。
4. `ReadingService<R>` の `R` はどこで具体化され、`BookRepository` の trait 境界は service に何を許しますか。
5. repository の所有値、共有参照、返される Future には、`Send` と `Sync` のどちらがなぜ必要ですか。
6. doctest、service のフェイク、SQLite Adapter、Router のテストは、それぞれ何を実際に保証し、何を保証しませんか。

### 総合解答

1. Axum の extractor が `CreateBookRequest` を作り、handler が 2 つの `String` を `CreateBook` へ移します。service はそれらを `BookTitle` と `Author` へ移し、所有したまま `insert_book` へ共有参照を渡します。SQLite Adapter はその参照から得た `&str` を SQLx に bind し、保存処理の完了まで借ります。DB から返った行は別の所有値として `StoredBook` に復元され、応答へ移ります。
2. service の Future が `BookTitle` と `Author` を所有し、その内側で repository の Future を完了まで待つためです。内側の Future は外側が保持する値より長く生存しません。別 task へ切り離していないので、これらの参照に無条件の `'static` は必要ありません。
3. `Book<Reading>` は、その所有値に `finish` を呼べることと、未読用の操作へ戻れないことをコンパイル時に保証します。入力文字列が既知の状態名か、取得した DB 行が有効か、取得時の状態が保存時にも一致するかは実行時に検査します。型状態だけでは、同じ ID の別の取得結果や同時更新を防ぎません。
4. 本番では `build_app` が `SqliteBookRepository` を `ReadingService::new` へ渡す式から `R = SqliteBookRepository` に決まります。service テストでは同じ位置で `FakeBookRepository` に決まります。`R: BookRepository` により、service は Adapter の内部を知らずに、trait が定める操作、結果、失敗へ依存できます。
5. repository の所有値には、service と状態をスレッド間で移動できるよう `Send` が必要です。repository を `&self` から共有し、その参照を待機中に保持するため `R: Sync` が必要です。各操作が返す Future 自体にも、Axum の handler まで入れ子になった処理を移動可能にする `Send` が必要です。いずれも実際に毎回別スレッドで実行するという保証ではありません。
6. doctest は公開された型で合法な遷移を書け、不正な遷移を書けないことを確かめます。フェイクは service の判断と、保存競合・保存失敗の伝播を制御して確かめます。SQLite Adapter のテストは実 SQL、行変換、条件付き保存を確かめます。Router のテストは routing から実 SQLite、HTTP 応答までを通しますが、TCP 設定や任意の並行実行順序までは保証しません。

答えの根拠を確かめ直すときは、[本の登録](../01-values/create-book.md)、[実行時の状態](../02-types/runtime-boundaries.md)、[ジェネリックな service](../03-abstraction/generic-service.md)、[非同期処理の借用](../04-async/async-bounds.md)、[状態変更の全体経路](status-update.md)へ戻ってください。
