# Issue tracker: GitHub

このリポジトリの Issue と仕様は GitHub Issues で管理する。すべての操作に `gh` CLI を使う。

## 規約

- Issue の作成: `gh issue create --title "..." --body "..."`。複数行の本文には heredoc を使う。
- Issue の参照: `gh issue view <number> --comments`。コメントを `jq` で絞り込み、ラベルも取得する。
- Issue の一覧: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`。用途に応じて `--label` と `--state` を指定する。
- Issue へのコメント: `gh issue comment <number> --body "..."`
- ラベルの追加と削除: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- Issue の終了: `gh issue close <number> --comment "..."`

対象リポジトリは `git remote -v` から判断する。クローン内で実行した場合、`gh` が自動的に解決する。

## Pull request をトリアージ対象として扱うか

PRs as a request surface: no.

`yes` に変更した場合、Pull request にも Issue と同じラベルと状態を適用し、対応する `gh pr` コマンドを使う。

- Pull request の参照: `gh pr view <number> --comments`。差分には `gh pr diff <number>` を使う。
- トリアージ対象となる外部 Pull request の一覧: `gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments` を実行し、`authorAssociation` が `CONTRIBUTOR`、`FIRST_TIME_CONTRIBUTOR`、`NONE` のものだけを残す。`OWNER`、`MEMBER`、`COLLABORATOR` は除外する。
- コメント、ラベル、終了: `gh pr comment`、`gh pr edit --add-label` / `--remove-label`、`gh pr close` を使う。

GitHub では Issue と Pull request が同じ番号空間を共有する。`#42` のような番号だけが渡された場合は、`gh pr view 42` を試し、該当しなければ `gh issue view 42` を使う。

## Skill が「issue tracker に公開する」と指示した場合

GitHub Issue を作成する。

## Skill が「関連チケットを取得する」と指示した場合

`gh issue view <number> --comments` を実行する。

## Wayfinding 操作

`/wayfinder` では、1 件の map Issue と、その配下の child Issue をチケットとして使う。

- Map: Notes、Decisions-so-far、Fog を本文に持ち、`wayfinder:map` ラベルを付けた単一の Issue。`gh issue create --label wayfinder:map` で作成する。
- Child ticket: GitHub の sub-issue として map にリンクした Issue。sub-issue API を `gh api` から使う。sub-issue が利用できない場合は map 本文のタスクリストに child を追加し、child 本文の先頭に `Part of #<map>` を記載する。ラベルは `wayfinder:<type>` を使い、`type` は `research`、`prototype`、`grilling`、`task` のいずれかとする。担当を開始したら、作業を進める開発者を assignee に設定する。
- Blocking: GitHub の native issue dependencies を正規かつ UI から確認できる表現として使う。`gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>` で依存関係を追加する。`<blocker-db-id>` は blocker の数値 database ID であり、`#number` や `node_id` ではない。`gh api repos/<owner>/<repo>/issues/<n> --jq .id` で取得する。GitHub は未解決の blocker を `issue_dependencies_summary.blocked_by` に返す。dependencies が利用できない場合は、child 本文の先頭に `Blocked by: #<n>, #<n>` を記載する。すべての blocker が閉じられた時点でチケットは unblocked になる。
- Frontier query: map 配下の未解決 child を map の順序で取得する。未解決 blocker がある child と assignee が設定済みの child を除外し、先頭の child を選ぶ。
- Claim: `gh issue edit <n> --add-assignee @me` を実行する。これはセッション最初の書き込み操作となる。
- Resolve: `gh issue comment <n> --body "<answer>"` で回答を記録し、`gh issue close <n>` で閉じる。その後、map の Decisions-so-far にコンテキストへの参照（gist とリンク）を追記する。
