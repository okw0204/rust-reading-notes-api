# フェイクと SQLite Adapter

## 問い

同じ `BookRepository` の契約を、SQLite とメモリ上のフェイクはどう満たし、`ReadingService<R>` の具体型はどこで決まるのでしょうか。

## 前提

[`BookRepository` の契約](repository-trait.md)を読み、型シグネチャだけでなく原子性や失敗後の状態も Interface に含まれることを確認しているものとします。

## 読む場所と順序

1. `src/service.rs` の `ReadingService<R>` と `impl<R: BookRepository>`。
2. `src/app.rs` の `build_app` と `build_app_with_repository`。
3. `src/repository/sqlite.rs` の `impl BookRepository for SqliteBookRepository`。
4. `src/test_support.rs` の `FakeBookRepository` と `record_reading_completion`。
5. `src/service/tests.rs` の保存失敗、並行進行、親 Future 終了のテスト。

```rust,ignore
{{#include ../../../src/service.rs:generic_service}}
```

```rust,ignore
{{#include ../../../src/app.rs:composition}}
```

## 解説

`R` は repository の型、`repository: R` は service が所有する値です。本番では `SqliteBookRepository::new(pool)` を `build_app_with_repository` へ渡す式から、`R = SqliteBookRepository` と推論されます。service テストでは同じ位置へ `FakeBookRepository` を渡します。

これは実行時に Adapter の一覧から選ぶ動的ディスパッチではありません。利用箇所ごとに具体化された `ReadingService<SqliteBookRepository>` と `ReadingService<FakeBookRepository>` が、同じ `impl<R: BookRepository>` のメソッド本体を使います。

SQLite Adapter は SQL、transaction、DB 行の変換を隠します。条件付き更新とメモ追加を実 DB で行い、成功時だけ両方を返します。

フェイクは SQLite の内部をまねません。`BTreeMap` 上で、呼び出し側から観測できる次の契約を同じにします。

- 現在状態が読書中の場合だけ本とメモを一緒に保存する。
- 競合や注入した失敗ではどちらも変えない。
- 一冊ごとの待機点で開始、完了、破棄を制御できる。

```rust,ignore
{{#include ../../../src/test_support.rs:fake_state}}
```

フェイクの clone は内部の `Arc` を clone し、service とテストが同じ保存状態を共有するためにあります。本や `BTreeMap` 全体を複製するわけではありません。短いメモリ操作のロック中に `.await` は挟みません。

| Adapter | 確認できること | これだけでは分からないこと |
| --- | --- | --- |
| SQLite | 実 SQL、transaction、行変換、migration と制約 | 特定の順序で Future を待機・解放した場合の service の判断 |
| フェイク | 失敗注入、並行開始、完了順、破棄後の保存状態 | SQL、SQLite の constraint、実 transaction |

テストは呼び出し回数や内部メソッドの順序ではなく、返る失敗と再取得できる保存状態を観測します。制御点は並行性や失敗位置を決定的に作るためにだけ使います。

## 別案との比較

### service が SQLx transaction を直接扱う

保存技術がユースケースへ漏れ、フェイクと SQLite で異なる Interface が必要になります。原子的保存は Adapter の内側へ置きます。

### 読了記録専用の repository trait を増やす

保存先も既存の差し替え先も同じです。新しい Seam を増やす実在の差異がないため、既存の `BookRepository` を深くします。

### `Box<dyn BookRepository>` を使う

実行時切り替えが必要なら候補ですが、現在の返り値は `impl Future` でそのまま dyn 互換ではありません。box 化と動的ディスパッチを加える要件もありません。

## 確認

1. 本番と service テストで `R` はどの式から決まりますか。
2. フェイクは SQLite の transaction 実装を再現する必要がありますか。
3. `fake.clone()` は何を共有しますか。
4. SQLite とフェイクの検証を分ける理由は何ですか。
5. 新しい repository trait を追加しない理由は何ですか。

## 解答

1. 本番は `SqliteBookRepository::new(pool)`、テストは `FakeBookRepository` を `ReadingService::new` や Router の組み立てへ渡す式です。
2. 必要ありません。成功時は本とメモの両方、失敗時はどちらも変更しないという観測可能な契約を満たします。
3. 内部の `Arc` が指す一つの `FakeState` への所有権を共有します。
4. 実 SQL と、決定的に制御した service の進行では確認できる範囲が異なるためです。
5. SQLite とフェイクという既存 Adapter が同じ本とメモの保存契約を担い、別の差し替え軸がないためです。

次は[Future が値を保持する範囲](../04-async/future-values.md)で、子 Future が何を所有し、何を親から借りるかを読みます。
