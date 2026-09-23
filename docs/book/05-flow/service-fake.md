# フェイクで service の判断を確かめる

## 問い

合法な遷移を計算できても、保存は失敗するかもしれません。service がその失敗を正しく返すことを、DB のタイミングに頼らずどう確かめるのでしょうか。

## 読む場所と順序

1. `src/service/tests/fake.rs` の `FakeBookRepository`、`FakeState`、`fail_next_update`。
2. 同ファイルの `update_book_status`。
3. `src/service/tests.rs` の `reports_a_save_conflict_without_changing_the_book` と `preserves_a_database_save_error_and_the_stored_state`。
4. 同ファイルの `rejects_invalid_inputs_without_repository_access` と `rejects_invalid_transitions_without_saving`。

```rust,ignore
{{#include ../../../src/service/tests/fake.rs:fake_state}}
```

## 解説

### 共有するのは制御可能な保存状態

フェイクは DB に接続せず、`BTreeMap` に本とメモを保存します。`Arc` によってテストと service が同じ `FakeState` を共有し、`Mutex` によってその変更を一度に一つの操作へ制限します。`fake.clone()` で別々の DB 相当の状態を複製するのではありません。

```rust,ignore
{{#include ../../../src/service/tests/fake.rs:fake_update}}
```

`lock()` から得たガードを保持する間に、呼び出し回数、注入エラー、保存条件を順に調べます。`take()` は `Option` の中身を取り出して `None` にするため、設定したエラーは次の更新で一度だけ返ります。エラーを返す枝は `books.insert` より前なので保存値を変更しません。

エラーを注入しない場合も、保存状態が `Some(expected)` と一致するか検査します。未検出なら `None` なので、取得後の削除も `Conflict` になります。フェイク自体の古い状態・削除後の更新テストもあり、成功を常に返すだけの代用品にはしていません。

### ロック中に await しない

各操作は `async fn` ですが、本文は短いメモリ操作だけで `.await` がありません。Future が実行されると、その操作は非同期の中断を挟まず完了し、ガードを解放します。呼び出し側に `.await` があっても、ガードを保持したまま停止する実装ではありません。

ここで使う `std::sync::Mutex` の `lock` は競合するとスレッドをブロックします。短い処理に閉じていることが、このフェイクでの利用を読む前提です。ガードを `.await` 越しに保持すると他の操作を長く待たせ、さらにこのガードは `Send` ではないので、停止中に保持する Future の `Send` 条件にも関わります。

### 保存の失敗を狙った場所で起こす

```rust,ignore
{{#include ../../../src/service/tests.rs:service_conflict_test}}
```

本の作成後にエラーを設定するので、`update_status` 内の取得は成功し、合法な遷移を計算した後の保存だけが失敗します。テストはエラーの種類、保存状態が未読のままであること、再試行で成功することを確認します。

`preserves_a_database_save_error_and_the_stored_state` は同じ仕組みで `AppError::Database(sqlx::Error::PoolClosed)` を注入します。実際に pool を閉じているわけではなく、service が保存エラーを保ったまま返すことを検査しています。対して API テストの `pool.close().await` は本物の pool を閉じ、HTTP で内部エラーを隠せるかを検査します。後者は一覧取得の失敗であり、保存箇所への失敗注入ではありません。

`calls()` の検査も結果だけでは見えない判断を確かめます。不正入力では0回、不正遷移では取得の1回だけ増え、保存まで進んでいないことが分かります。フェイクの目的は SQLite 全体の再現ではなく、service の分岐とエラー伝播を制御して確かめることです。

## 確認

1. `fake.clone()` と `Arc<Mutex<_>>` が必要なのは何を共有するためですか。
2. エラーが一度だけ返り、保存状態が変わらない根拠はどこですか。
3. フェイクの `.await` 中に `Mutex` のガードが保持されたまま中断しますか。
4. `PoolClosed` の注入と API テストの `pool.close()` は同じ経路を検査していますか。

## 解答

1. service に所有させたフェイクと、テスト側の失敗設定・検査用ハンドルで同じ保存状態を共有するためです。
2. `next_update_error.take()` が設定を消費し、`books.insert` より前に `Err` を返します。テストは保存値と再試行も確認しています。
3. しません。ロックを取得する各メソッドの本文には `.await` がなく、短い操作の終わりにガードを解放します。
4. 違います。フェイクは service の保存失敗を狙い、API は実際の DB アクセス失敗から HTTP の500への変換を確かめます。

実際の SQL の保証は、次の[SQLite の契約テスト](sqlite-contract.md)で読みます。
