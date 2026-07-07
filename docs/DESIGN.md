# init-env

- コマンドラインオプションやサブコマンドは持たない
- 入力はすべて対話的に受け取る

## 行うこと

nix-config の `init-gh-repo` を代替する。

- owner（デフォルト `gawakawa`）・リポジトリ名・公開設定・flake テンプレート・Secrets 設定・branch rule 設定を対話的に受け取る
- flake テンプレートの一覧は `gawakawa/flake-templates` リポジトリから取得する
- GitHub リポジトリを作成し clone する
- 選択した flake テンプレートを適用して push する
- 作成したリポジトリに branch rule を設定する
