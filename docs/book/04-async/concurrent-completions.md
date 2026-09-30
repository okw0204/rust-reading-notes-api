# 独立した読了記録を並行に進める

## 問い

一括読了記録は、なぜ一冊目の保存が終わるまで二冊目を待たずに進めるのでしょうか。複数の Future を同じ親 Future の中で進める実装と、同時に進める仕事量の上限を読みます。

## 前提

[Future が値を保持する範囲](future-values.md)まで読み、`async fn` の呼び出しが Future を返すこと、子 Future が再開に必要な所有値と借用を保持することを確認しているものとします。

## 読む場所と順序

1. [子 Future の集合を作る](#子-future-の集合を作る) で `ReadingService::record_reading_completions` が `into_iter`、`enumerate`、`map`、`collect` で作る `FuturesUnordered`。
2. [作るだけでは進まない](#作るだけでは進まない) で `while let ... next().await` が集合を進める仕組み。
3. [上限は要求の不変条件に置く](#上限は要求の不変条件に置く) で件数検証との関係。
4. [並行進行を実時間で推測しない](#並行進行を実時間で推測しない) で二冊の開始を観測するフェイクとテスト。

## 解説

### 子 Future の集合を作る

対象: `src/service.rs` / `ReadingService::record_reading_completions`

```rust,ignore
{{#include ../../../src/service.rs:reading_completions_pipeline}}
```

1. `validated.into_iter()` は検証済みの `Vec` を消費し、各 `(book_id, body)` を値で取り出します。`iter()` は要素を借用するのに対し、`into_iter()` は要素そのものを渡すため、ここでは子 Future へ所有権を移せます。
2. `.enumerate()` は各要素へ 0 から始まる `position` を付け、`(position, 要素)` の組にします。先頭の入力が位置 0、次が位置 1 です。
3. `.map(|(position, (book_id, body))| async move { ... })` は、各要素を子 Future へ変換します。クロージャの引数は分解パターンで、組から位置と入力値を一度に取り出します。
4. `.collect::<FuturesUnordered<_>>()` は `Iterator<Item = Future>` を `FuturesUnordered` へ集めます。`_` は要素の Future 型をコンパイラが推論することを示し、外側の型だけを指定しています。

子 Future の完了時の値は `(position, result)` の組です。`result` をどう作るかは[失敗と親 Future の終了](failure-and-parent-future.md)で読みます。

`FuturesUnordered` は、保持している Future のうち進行可能になったものを poll し、完了した結果から `next()` で返します。一つの子 Future が `Pending` になっても、親 Future はほかの子 Future を poll できます。これは複数の処理を並行に進める仕組みであり、別スレッドで同時に実行されることまで保証するものではありません。

設計理由: 入力一覧を `into_iter()` で消費してから子 Future へ移すと、子 Future の内側に所有値が集まり、親は位置と結果だけを持つ `FuturesUnordered` を管理すればよくなります。

### 作るだけでは進まない

対象: `src/service.rs` / `ReadingService::record_reading_completions`

```rust,ignore
{{#include ../../../src/service.rs:reading_completions_drain}}
```

1. `Vec::with_capacity(pending.len())` は、結果が最大でも集合と同じ個数だと分かっているので、置き場を先に確保します。
2. `while let Some(result) = pending.next().await` は、完了値が残っている間だけ本体を繰り返すループです。`next()` は `Stream` のメソッドで、完了した値があれば `Some`、集合が空になれば `None` を返します。
3. `pending.next().await` の `await` が、親 Future を poll するたびに集合内の子 Future を進めます。
4. `results.push(result)` は完了した順に組を集めます。この時点では入力順になっていません。並べ直しは[完了順と入力順を分ける](completion-order.md)で読みます。

`map` と `collect` は子 Future を作って `FuturesUnordered` へ所有させるだけです。この時点では repository 操作が完了したわけではありません。親 Future が `pending.next().await` を待って初めて、集合内の子 Future が poll されます。

全件を作ったあとで最初の `next()` を待つため、一件目を `.await` してから二件目を作る逐次処理にはなりません。`tokio::spawn` で切り離した task は作らないため、処理を進めるためだけに入力や repository を `'static` にする必要もありません。親 Future が子 Future の集合を所有し、その場で全件の完了を待ちます。

### 上限は要求の不変条件に置く

一つの要求は 1 件以上 8 件以下です。この件数検査は `validated` を作る前に走るため、親 Future が一度に所有する子 Future も最大 8 個になります。検査そのものは[要求から検証済みの入力へ](../01-values/reading-completion-input.md)で読んでいます。

9 件の要求は `400 Bad Request` になり、`rejects_completion_requests_above_the_concurrency_limit` が利用者向けの HTTP Interface から上限を確認します。この検証が何を示し、何を示さないかは[検証の保証と限界](../05-flow/verification-limits.md)で扱います。

別の semaphore や worker queue は置いていません。最大 8 件という既存の要求上限だけで、一要求から無制限に仕事が作られることを防げるためです。より大きな要求、要求間の公平性、DB 接続数に合わせた厳密な制御が必要になれば、この判断は変わります。

### 並行進行を実時間で推測しない

対象: `src/test_support.rs` / `ReadingCompletionControl`

```rust,ignore
{{#include ../../../src/test_support.rs:completion_gate}}
```

1. `wait_for_release` は、本ごとの `Semaphore` の permit が来るまで `.await` します。
2. フェイクは保存処理の中でこのメソッドを待つので、permit が来るまで対象の一冊は `Pending` を返します。

対象: `src/test_support.rs` / `ReadingCompletionControl`

```rust,ignore
{{#include ../../../src/test_support.rs:completion_release}}
```

1. `release` は該当する gate へ `add_permits(1)` を呼ぶだけです。
2. 待っている処理を直接再開させるのではなく、permit を足して次の poll で進める状態にします。

対象: `src/service/tests.rs` / `advances_completions_concurrently_and_returns_results_in_input_order`

```rust,ignore
{{#include ../../../src/service/tests.rs:completion_order_drive}}
```

1. `control.wait_for_started(2).await` は、二冊の保存処理がどちらも開始したと分かるまで待ちます。
2. `assert!(control.finished().is_empty())` は、まだ完了が一件もないことを確かめます。
3. `control.release(second.id())` と続く `wait_for_finished(1)` は、二冊目だけを先に完了させます。
4. `control.release(first.id())` で一冊目も解放します。

この検証は「逐次実装より何ミリ秒短かったか」を測りません。実行環境の速度や scheduler の偶然ではなく、解放前に複数の処理が開始済みという状態を観測します。逐次実装なら一冊目の待機中に二冊目へ到達できないため、この条件を満たせません。有限のテストから任意の実行順は保証できませんが、service が不要な逐次待機を加えていないことは確認できます。

## 比較した別案

### 一件ずつ await する

実装は単純で、最終的な保存結果も同じにできます。しかし一冊の I/O 待機中に独立した本を進められず、「複数の独立した読了記録を進行可能にする」という Interface を満たしません。

### tokio::spawn で task を分ける

各処理を独立した task として実行する要件があれば成立します。今回は HTTP の親 Future が全結果を待ち、終了時には子 Future も終了する契約です。`spawn` を使うと `Send + 'static`、task の所有者、終了後の結果の扱いという追加の問題が生じるため採用しません。

### 無制限に Future を作る

入力件数に上限がなければ、要求サイズに応じて Future と DB 操作を無制限に増やします。現在は 8 件という要求の不変条件が上限として働くため、追加の仕組みは不要です。

## 確認

1. 一冊の処理内では順序が必要でも、冊子間では順序が不要なのはなぜですか。
2. `into_iter` と `iter` のどちらを使うかで、子 Future が受け取る値はどう変わりますか。
3. `FuturesUnordered` を作った時点で repository 操作は完了していますか。
4. この実装が `tokio::spawn` を必要としないのはなぜですか。
5. 同時に進める仕事量が 8 件を超えないことは、どの Interface から確認できますか。
6. テストは並行進行を実行時間ではなく何から判断しますか。

## 解答

1. 一冊では取得した状態を検査して遷移させ、その結果を保存するためです。冊子間は重複 ID を拒否した独立した処理で、一冊の結果を別の一冊が使いません。
2. `into_iter` は `Vec` を消費して `book_id` と `body` を子 Future へ移します。`iter` なら借用を渡すことになり、親が入力一覧を保持し続ける必要があります。
3. していません。Future を集合へ入れただけで、`next()` を待つ親 Future が poll して初めて処理が進みます。
4. 親 Future が子 Future の集合を所有し、その場で完了を待てるためです。呼び出し元から独立した `'static` な task は必要ありません。
5. `POST /reading-completions` が 9 件を `400 Bad Request` で拒否することと、検証後の各入力から一つずつ子 Future を作る実装から確認できます。
6. フェイクが待機中の二冊について、どちらも解放する前に開始済みになったことから判断します。

次は[完了順と入力順を分ける](completion-order.md)で、先に終わった結果をそのまま返さず、利用者の入力へ対応付け直す仕組みを読みます。
