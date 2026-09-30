# フェイクと SQLite Adapter

## 問い

同じ `BookRepository` の契約を、SQLite とメモリ上のフェイクはどう満たし、`ReadingService<R>` の具体型はどこで決まるのでしょうか。

## 前提

[`BookRepository` の契約](repository-trait.md)を読み、型シグネチャだけでなく原子性や失敗後の状態も Interface に含まれることを確認しているものとします。

## 読む場所と順序

1. [型パラメータ `R` は利用箇所で決まる](#generic-r)（`src/service.rs` の `ReadingService<R>`）。
2. [アプリの組み立ては Adapter を一つ受け取る](#app-assembly)（`src/app.rs` の `AppState<R>` と `build_app_with_repository`）。
3. [FakeState は共有所有と排他で作る](#fake-shared)（`src/test_support.rs` の `FakeBookRepository` と `FakeState`）。
4. [一冊ごとの進行を制御する](#fake-control)（`ReadingCompletionControl` と失敗注入の API）。
5. [フェイクは同じ保存契約を満たす](#fake-contract)（`update_book_status` と `record_reading_completion`）。

## 解説

<a id="generic-r"></a>
### 型パラメータ `R` は利用箇所で決まる

対象: `src/service.rs` / `ReadingService<R>`

```rust,ignore
{{#include ../../../src/service.rs:generic_service}}
```

- `ReadingService<R>` の `R` が repository の具体型です。service はこの型を型引数として受け取り、`repository: R` として所有します。
- `impl<R: BookRepository>` の境界が、`R` に要求する能力を一つに絞ります。`BookRepository` を実装していれば、SQLite でもフェイクでも同じメソッド本体を使えます。
- `new(repository: R)` は値を受け取ってそのまま所有します。`Box<dyn ...>` のような間接参照も動的ディスパッチも伴いません。

本番では `SqliteBookRepository::new(pool)` を `ReadingService::new` へ渡す式から `R = SqliteBookRepository` と推論されます。service テストでは同じ位置へ `FakeBookRepository` を渡すため `R = FakeBookRepository` になります。これは実行時に Adapter の一覧から選ぶ動的ディスパッチではなく、利用箇所ごとに具体化された `ReadingService<SqliteBookRepository>` と `ReadingService<FakeBookRepository>` が同じ `impl<R: BookRepository>` のメソッド本体を共有する形です。

<a id="app-assembly"></a>
### アプリの組み立ては Adapter を一つ受け取る

Router が全 handler で共有する状態です。

対象: `src/app.rs` / `AppState<R>`

```rust,ignore
{{#include ../../../src/app.rs:app_state}}
```

- `AppState<R>` は `Arc<ReadingService<R>>` を一つ持ちます。`Arc` は複数の handler が同じ service を共有所有するためにあり、`R` を隠しません。
- `Clone` は `Arc::clone(&self.service)` を呼ぶだけです。service や repository を作り直さず、参照カウントを一つ増やします。
- `R` はここでも型引数のままです。`AppState<SqliteBookRepository>` と `AppState<FakeBookRepository>` のどちらにもなれます。

Adapter を受け取って状態と Router を組む入口です。

対象: `src/app.rs` / `build_app`、`build_app_with_repository`

```rust,ignore
{{#include ../../../src/app.rs:app_build}}
```

- `build_app` は `SqliteBookRepository::new(pool)` を渡す本番用の入口です。ここで `R = SqliteBookRepository` が決まります。
- `build_app_with_repository<R>` は Adapter を引数で受け取る汎用の入口です。境界は `R: BookRepository + 'static` で、`+ 'static` は Router の状態が持つ寿命の要求です。
- `Arc::new(ReadingService::new(repository))` が、所有した repository を service へ渡し、service を共有所有へ包む一文です。型決定の中心はここにあります。

各経路の handler にも同じ `R` を渡します。

対象: `src/app.rs` / `build_app_with_repository`（ルート配線）

```rust,ignore
{{#include ../../../src/app.rs:completion_route}}
```

- `handler::record_reading_completions::<R>` の turbofish が、handler の型パラメータを Router の `R` に固定します。実行時の分岐ではなくコンパイル時に決まります。

<a id="fake-shared"></a>
### FakeState は共有所有と排他で作る

フェイクの外側の型です。

対象: `src/test_support.rs` / `FakeBookRepository`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_struct}}
```

- `state: Arc<Mutex<FakeState>>` が保存状態の置き場所です。`Arc` は service 側とテスト側で同じ `FakeState` を共有所有するためにあります。
- `#[derive(Clone)]` の clone は `Arc` を clone し、参照カウントを増やすだけです。`BTreeMap` 全体を複製しません。
- `Mutex` は parking_lot の同期プリミティブで、複数の子 Future が同じ `FakeState` へ触れる間の排他を担います。

共有される保存状態の中身です。

対象: `src/test_support.rs` / `FakeState`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_state_fields}}
```

- `books` と `notes` は `BTreeMap` で、SQLite の表の代わりに本とメモを保持します。`next_book_id` と `next_note_id` が採番を担います。
- `next_update_error` と `reading_completion_errors` は、次に起こす失敗をテストから注入する場所です。実 DB を故障させずに失敗経路を作ります。
- `reading_completion_control` は、並行進行を決定的に制御したい場合だけ差し込む待機点です。

<a id="fake-control"></a>
### 一冊ごとの進行を制御する

待機点の共有と破棄の記録です。

対象: `src/test_support.rs` / `ReadingCompletionControl`、`ReadingCompletionAttempt`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_control_wrapper}}
```

- `ReadingCompletionControl` は `Arc<ReadingCompletionControlInner>` を持ち、clone で同じ制御状態を共有します。
- `ReadingCompletionAttempt` は `Drop` で自分が完了前に捨てられたかを記録します。親 Future の終了で子が破棄されたことをテストから観測できます。

待機点が持つ状態です。

対象: `src/test_support.rs` / `ReadingCompletionControlInner`、`ReadingCompletionProgress`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_control_state}}
```

- `gates: BTreeMap<i64, Arc<Semaphore>>` は本の ID ごとに許可証 0 枚の `Semaphore` を置きます。`release` で許可証を 1 枚足すまで、その本の読了記録は待ちます。
- `progress` と `changed: Notify` は、開始・完了・破棄のどれが起きたかを記録し、テストへ通知します。

テストが見る待機 API です。

対象: `src/test_support.rs` / `ReadingCompletionControl`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_control_wait}}
```

- `wait_for_started` と `wait_for_finished` は、指定件数が始まる・終わるまで `Notify` で待ちます。時間ではなく件数を条件にします。

解放と観測の API です。

対象: `src/test_support.rs` / `ReadingCompletionControl`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_control_release}}
```

- `release` は特定の本の `Semaphore` に許可証を足し、待っていた読了記録を先へ進めます。
- `finished` と `dropped` は、完了した本と破棄された本をテストへ返します。

失敗注入の入口です。

対象: `src/test_support.rs` / `FakeBookRepository`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_inject_errors}}
```

- `fail_next_update` は次の状態更新を一度だけ失敗させます。`fail_reading_completion` は本の ID を指定して読了記録を失敗させます。

待機点の差し込みです。

対象: `src/test_support.rs` / `FakeBookRepository::control_reading_completions`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_control_setup}}
```

- 対象の本ごとに許可証 0 枚の `Semaphore` と `progress`、`Notify` を作り、フェイクの状態へ差し込みます。呼び出しはテストの準備段階だけです。

<a id="fake-contract"></a>
### フェイクは同じ保存契約を満たす

条件付き更新の実装です。

対象: `src/test_support.rs` / `FakeBookRepository::update_book_status`

```rust,ignore
{{#include ../../../src/test_support.rs:fake_update}}
```

- 注入された `next_update_error` があれば `take` で一度だけ取り出し、保存の前に返します。
- `state.books.get(&id).map(StoredBook::status) != Some(expected)` で、現在状態が取得時の状態と一致するか調べます。不一致なら `Conflict` を返し、`BTreeMap` を書き換えません。
- 一致するときだけ `state.books.insert(id, next.clone())` で遷移済みの本を保存します。

待機点を持つ読了記録の前半です。

対象: `src/test_support.rs` / `FakeBookRepository::record_reading_completion`（待機）

```rust,ignore
{{#include ../../../src/test_support.rs:fake_completion_gate}}
```

- `control_reading_completions` で待機点が差し込まれていれば、`start` で開始を記録し、`wait_for_release(book_id).await` でその本の許可証を待ちます。
- `self.state.lock()` の結果は文の終わりで解放されるため、ロックを保持したまま `.await` しません。小さなメモリ操作の間だけ排他します。

保存部です。

対象: `src/test_support.rs` / `FakeBookRepository::record_reading_completion`（保存）

```rust,ignore
{{#include ../../../src/test_support.rs:fake_completion_save}}
```

- 注入された読了記録の失敗を先に取り出し、あれば保存せずに返します。失敗経路では状態もメモも変えません。
- 現在状態が `ReadingStatus::Reading` でなければ `Conflict` です。SQLite の `WHERE status = 'reading'` と同じ条件をメモリ上で表します。
- 条件を満たすときだけ、状態を `Finished` へ更新し、メモを同じ `state` の変更として追加します。片方だけを残す中間状態がありません。
- 最後に `attempt.finish()` を呼び、破棄ではない完了として記録します。親 Future が途中で捨てられれば `Drop` が破棄として記録します。

フェイクは SQLite の内部をまねません。`BTreeMap` 上で、呼び出し側から観測できる契約を同じにします。テストは呼び出し回数や内部メソッドの順序ではなく、返る失敗と再取得できる保存状態を観測します。制御点は並行性や失敗位置を決定的に作るためにだけ使います。

`Arc`、`Mutex`、`Semaphore` はスレッドをまたいで使われるため、その内側の型の `Send` と `Sync` が要求に関わります。これらと Future の寿命の関係は[Future が値を保持する範囲](../04-async/future-values.md)で読みます。

## 別案との比較

### service が SQLx transaction を直接扱う

保存技術がユースケースへ漏れ、フェイクと SQLite で異なる Interface が必要になります。原子的保存は Adapter の内側へ置きます。

### 読了記録専用の repository trait を増やす

保存先も既存の差し替え先も同じです。新しい Seam を増やす実在の差異がないため、既存の `BookRepository` を深くします。

### `Box<dyn BookRepository>` を使う

実行時切り替えが必要なら候補ですが、現在の返り値は `impl Future` でそのまま dyn 互換ではありません。box 化と動的ディスパッチを加える要件もありません。

## 確認

1. 本番と service テストで `R` はどの式から決まりますか。
2. `AppState<R>` の clone は何を複製しますか。
3. `fake.clone()` は何を共有しますか。
4. `reading_completion_control` で `Semaphore` を使うのは何のためですか。
5. フェイクは SQLite の transaction 実装を再現する必要がありますか。
6. SQLite とフェイクの検証を分ける理由は何ですか。

## 解答

1. 本番は `SqliteBookRepository::new(pool)` を `build_app` の位置へ渡す式、テストは `FakeBookRepository` を `ReadingService::new` や Router の組み立てへ渡す式です。
2. `Arc::clone` により、`ReadingService<R>` への参照カウントだけを増やします。service や repository は作り直しません。
3. 内部の `Arc` が指す一つの `FakeState` への所有権を共有します。`BTreeMap` 全体を複製しません。
4. 読了記録を、待ち時間ではなくテストが指定した時点で解放するためです。開始と完了の順序を決定的に作れます。
5. 必要ありません。成功時は本とメモの両方、失敗時はどちらも変更しないという観測可能な契約を満たします。
6. 実 SQL と、決定的に制御した service の進行では確認できる範囲が異なるためです。

次は[Future が値を保持する範囲](../04-async/future-values.md)で、子 Future が何を所有し、何を親から借りるかを読みます。
