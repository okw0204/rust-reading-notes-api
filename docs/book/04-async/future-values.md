# Future が値を保持する範囲

## 問い

一冊の読了記録を表す Future は何を所有し、何を借用して `.await` をまたぐのでしょうか。`Send`、`Sync`、`'static` の要求元も分けます。

## 前提

第 3 部まで読み、`ReadingService<R>` が repository を所有し、`BookRepository` の操作が Future を返すことを確認しているものとします。

## 読む場所と順序

1. [子 Future が所有する値と借りる値](#子-future-が所有する値と借りる値) で `ReadingService::record_reading_completions` が作る子 Future。
2. [借用を await またぎで保つ](#借用を-await-またぎで保つ) で `ReadingService::record_reading_completion` が repository Future へ貸す参照。
3. [Send と Sync の要求元](#send-と-sync-の要求元) で `BookRepository` の宣言と `AppState<R>` の所有。
4. [親 Future までの入れ子](#親-future-までの入れ子) で handler の Future が service の Future を await する形。

## 解説

### 子 Future が所有する値と借りる値

対象: `src/service.rs` / `ReadingService::record_reading_completions`

```rust,ignore
{{#include ../../../src/service.rs:reading_completions_child}}
```

1. `map` に渡す `|(position, (book_id, body))| ...` はクロージャです。一覧の一要素を分解し、`position`、`book_id`、`body` を受け取ります。
2. `async move` は、このクロージャが受け取った値を子 Future の内部へ移動させます。`book_id` と `body` の所有者は、親の入力一覧ではなく子 Future になります。
3. 本体は `self.record_reading_completion(book_id, &body).await` を呼びます。`book_id` は値で渡し、`body` は `&body` として借ります。
4. 戻り値は `(position, result)` の組です。子 Future は完了時にこの組を親へ渡します。

`async fn` の呼び出しは処理を完了させず、Future を返します。実行器が poll し、内側の Future が `Pending` なら、再開に必要な値を保持して制御を返します。子 Future は `book_id`、`body`、`position` を自分の内側に持つため、親が制御を返している間も値が生き続けます。`.await` は新しいスレッドを作る命令でも、必ず停止する命令でもありません。

```text
handler Future
  owns AppState<R>
  awaits parent service Future
    owns validated inputs and FuturesUnordered
      child Future
        owns position, book_id, NoteBody
        borrows ReadingService<R> and repository
        awaits repository Future borrowing NoteBody
```

設計理由: 検証後に確定した所有値を clone せず子 Future へ移すと、値の持ち主と実行中の処理が一致します。親の入力一覧を借用したまま待つ形にすると、親が `validated` を破棄した後の待機で借用が続き、どの値がいつまで有効かを追いにくくなります。

### 借用を await またぎで保つ

対象: `src/service.rs` / `ReadingService::record_reading_completion`

```rust,ignore
{{#include ../../../src/service.rs:single_completion_future}}
```

1. 引数の `body: &NoteBody` は借用です。このメソッド自身は本文を所有しません。
2. `self.repository.find_book(book_id).await?` は repository が返す Future を待ちます。
3. `self.repository.record_reading_completion(reading.finish(), body).await?` へ `body` をそのまま渡します。repository Future がこの参照を借ります。
4. `&self` も借用です。メソッド全体の Future は、service とその内側の repository を待機中も借り続けます。

`body` の参照元は、子 Future が所有する `NoteBody` です。repository Future は `body` を借りるので、子 Future より長生きできません。子 Future の内側で完了まで待ち、外へ切り離さない形なので、repository の返り値は `impl Future<...> + Send` でも無条件の `+ 'static` を持たないのです。

設計理由: Future を所有する側と借用される側を入れ子にすると、待機中の参照がどこに属するかを型で追えます。参照を `'static` にして切り離すには clone か `Arc` が必要になり、その分だけ値の流れが増えます。

### Send と Sync の要求元

対象: `src/repository.rs` / `BookRepository` の宣言と `record_reading_completion`

```rust,ignore
{{#include ../../../src/repository.rs:trait_header}}
{{#include ../../../src/repository.rs:record_completion_contract}}
```

1. `trait BookRepository: Send + Sync` の `Send` は、repository を所有する service をスレッド間で移せる条件です。
2. `Sync` は、複数の子 Future が `&R` を共有し、待機中にその借用を保持できる条件です。
3. 各メソッドの `impl Future<...> + Send` は、返る Future をスレッド間で移せることを表します。trait のメソッドで `async fn` と書くとこの `Send` を型に表せないため、`impl Future + Send` を明示しています。
4. 戻り値の型に `'static` は書かれていません。Future の有効期間は、借りている `&self` と `&NoteBody` の範囲に結び付きます。

`Send` は実際に別スレッドで実行される保証ではありません。実行器が同じスレッドで poll することもあります。それでも、処理の途中状態を別スレッドへ移す可能性があるため、Axum と Tokio は handler まで入れ子になった Future に `Send` を要求します。

対象: `src/app.rs` / `AppState` と `build_app_with_repository`

```rust,ignore
{{#include ../../../src/app.rs:app_state}}
{{#include ../../../src/app.rs:app_build}}
```

1. `AppState<R>` は `Arc<ReadingService<R>>` を所有し、handler へ `Clone` で渡ります。`Arc::clone` は中身を複製せず、service の共有所有を増やします。
2. Router の状態には `Clone + Send + Sync + 'static` が要求されます。`AppState<R>` がこの条件を満たすには、内側の service と repository にも `Send` と `Sync` が必要です。
3. `AppState<R>` の `'static` は Router が状態を長期に保持するための要求で、子 Future が repository を借用する期間とは別の要求です。

`Arc` は共有所有を提供しますが、内側の型を自動でスレッド安全にはしません。共有する repository の `Sync` と、返る Future の `Send` は、それぞれ別の場所で要求されます。

### 親 Future までの入れ子

対象: `src/handler.rs` / `record_reading_completions`

```rust,ignore
{{#include ../../../src/handler.rs:completion_handler_signature}}
{{#include ../../../src/handler.rs:completion_handler_input}}
```

1. handler の Future は `State<AppState<R>>` と `Json(request)` を所有します。
2. `request.items.into_iter().map(...).collect()` は、HTTP の DTO を service の入力 `RecordReadingCompletions` へ詰め替えます。`item.body` の `String` は新しい構造体へ移動します。
3. handler は service の Future を `.await` するため、handler の Future の内側に service と子 Future が入れ子になります。
4. 入れ子全体を移動できる必要があるので、子 Future の `Send` が handler まで伝わります。

設計理由: 親 Future が子 Future を所有して内側で待つ形は、`'static` な task を作らずに済みます。値の寿命が呼び出しの入れ子に沿って短くなるほど、所有関係も短く追えます。

## 別案との比較

### `tokio::spawn` で子 task を切り離す

`spawn` は Future に `Send + 'static` を要求します。今回は親 Future が全結果を待ち、終了時に未完了の子も終了する契約なので、task の所有者や永続的な処理状態を増やしません。

### `Arc<NoteBody>` を各子へ渡す

各本文は一つの子 Future だけが使います。共有所有者を増やす必要がなく、`NoteBody` を値で移す方が小さく追えます。

### 借用した HTTP DTO を子 Future に残す

親が DTO を保持し続ければ成立する形もありますが、検証済みの値と未検証の入力の境界が曖昧になります。検証後の所有値へ切り替えます。

## 確認

1. 子 Future が所有する値と借用する値は何ですか。
2. repository Future に無条件の `'static` が不要なのはなぜですか。
3. `R: Sync` と返る Future の `Send` は、同じ条件ですか。
4. `Arc` だけで任意の repository を安全に共有できますか。
5. `tokio::spawn` を使うと追加で何を設計する必要がありますか。

## 解答

1. 入力位置、`book_id`、`NoteBody` を所有し、親が保持する service と repository を共有借用します。
2. 参照先を所有する子 Future の内側で repository Future を完了まで待ち、外へ切り離さないためです。
3. 違います。`Sync` は `&R` の共有に、Future の `Send` は待機中の状態全体の移動可能性に関係します。
4. できません。`Arc` は共有所有だけを担い、内側の `Send` と `Sync` の条件は残ります。
5. task の所有者、結果の再取得、shutdown、再試行、親の終了後と HTTP 応答の関係が必要です。

次は[独立した読了記録を並行に進める](concurrent-completions.md)で、これらの子 Future を同時に進行可能にする仕組みを読みます。
