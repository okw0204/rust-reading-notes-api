# SQLite で保存の契約を確かめる

## 問い

競合を確かめるために、二つのタスクを同時に走らせる必要はあるでしょうか。DB から取得した値と現在の保存状態を分け、テストが作る順序を追います。

## 読む場所と順序

1. `src/repository/sqlite.rs` の `conditional_update_rejects_a_stale_state`。
2. 同ファイルの `update_book_status` と `conditional_update_rejects_deletion_after_fetch`。
3. 同ファイルの行変換、保存文字列の検査、`rejects_an_unknown_stored_status`。
4. 同ファイルの `insert_note` と `inserting_a_note_for_an_unknown_book_returns_not_found`、`migrations/` の制約。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:stale_update_test}}
```

## 解説

### 二つの取得結果を順に保存する

`#[sqlx::test(migrations = "./migrations")]` は SQLx がテストごとに独立した SQLite のファイル DB と pool を準備し、アプリと同じ migration を適用して `SqlitePool` を渡します。`connect_database` は呼ばず、その1接続設定も引き継ぎません。[API テスト](http-integration.md)の準備関数が使うインメモリ DB（`sqlite::memory:`）・1接続とは異なりますが、どちらも手元の `reading-notes.db` を共有しません。

ここでは実際の SQLite repository を使うので、SQL の条件・行の取得・ドメイン型への変換まで通ります。

テストは同じ未読の本を二度取得し、別々に所有する `first` と `second` を作ります。先に `first` を読書中として保存すると DB は読書中になります。しかし `second` は取得時の未読の値のままです。その古い値から遷移を作り、`expected = WantToRead` で保存すると失敗します。

この順序は「両者が未読を取得し、一方が先に保存した」という競合の核心を再現します。並列実行のタイミングや待ち時間の調整は必要ありません。テストは `Conflict` に加え、DB が読書中のままであることも確認します。ただし、これであらゆる並列スケジュールを検査しているわけではありません。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:conditional_update}}
```

状態の比較と書き込みは一つの `UPDATE` に含まれます。取得後に削除するテストでも行が返らず `Conflict` になり、その後の通常取得は `NotFound` になります。取得時の404と条件付き保存時の409という契約は、同じ未検出でも操作の段階で区別されます。

### 接続数と原子性を分ける

この repository テストの pool は1接続に固定していません。一方、アプリや API テストのように最大1接続に制限しても、`find_book(...).await` から `update_book_status(...).await` までの service 全体が一つの操作になるわけではありません。接続を返した後、別の要求が使う余地があります。複数の `.await` をまたぐ一連の処理を、接続数だけで原子的とは呼べません。

メモの追加では、存在確認と追加を一つの文にまとめています。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:atomic_note_insert}}
```

`INSERT ... SELECT ... WHERE EXISTS` は、親の本がある場合だけメモを追加します。別々の `SELECT` と `INSERT` の間に削除が入り込む隙間を、この SQL 文の中には作りません。該当しなければ `RETURNING` の行がないので `NotFound` です。外部キーの制約と併せて保存の整合性を読みます。未知の本への追加テストはこの未検出を確認しますが、並列の削除を実行しているテストではありません。

### DB と行変換の守備範囲

`BookRow` や `NoteRow` は DB の文字列を保持し、`TryFrom` で検証済みの型へ変換します。実 DB テストは空白だけの書名・著者・メモを SQL で保存し、repository から読むと `InvalidStoredValue` になることを確認します。HTTP の入力検証を経ない保存値にも復元境界で検証が必要です。

未知の状態名は migration の `CHECK` が通常の保存時に拒否します。`rejects_an_unknown_stored_status` は DB にその値を挿入するテストではなく、`BookRow { status: "paused", ... }` を直接作る単体テストです。行変換自体も未知の状態を拒否することを確かめます。DB 制約を通った実行経路と、変換境界単体の検査を混同しないでください。

## 確認

1. 競合テストが二つのタスクを並列起動しないのはなぜですか。
2. pool が1接続なら、service の取得から保存までは原子的ですか。
3. メモの存在確認を独立した SQL にせず、追加文に含める理由は何ですか。
4. 未知の保存状態の単体テストは、通常の SQLite に未知の値を保存できることを示しますか。

## 解答

1. 二つの取得結果を持ち、一方を保存した後にもう一方を保存すれば、古い期待状態の拒否を決まった順序で再現できるためです。
2. いいえ。複数の SQL の間には別の処理が入り得ます。条件の比較と書き込みを一つの SQL に含めることとは別です。
3. 存在確認と追加の間に削除が入る隙間を避け、存在しなければ行を返さず `NotFound` にするためです。
4. いいえ。通常は DB の `CHECK` が防ぎます。単体テストは手作りの行を使い、変換境界の拒否を確かめています。

HTTP まで含めた観測は[Router の統合テスト](http-integration.md)へ進みます。
