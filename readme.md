# Shotclassif

## 概要

ShotclassifはTUIでの画像整理ツールです．

キーボードショートカットを用いて高速に画像を複数のフォルダに分類することができます．

## 使い方

### インストール

#### バイナリを利用 (Windowsのみ)

Releaseページから最新版の.exeファイルをダウンロードしてください．


#### コードからビルド

ソースコードからビルドするにはRustツールチェーンが必要です．

```
git clone https://github.com/hinshiba/shotclassif.git
cd shotclassif
cargo build --release
```

### 実行

実行ファイルと同じディレクトリに`config.toml`という名前で設定ファイルを作成してください．
```toml
# 分類したい画像が格納されているディレクトリ
dir = "C:/Users/YourUser/Pictures/Unsorted"

# キーと分類先ディレクトリのマッピング
[dists]
# "a"キーを押すと"hoge/huga"に移動
"a" = "hoge/huga"
# "b"キーを押すと "./temp" に移動
"b" = "./temp"
# 移動先に"skip"を指定すると移動せずにスキップします
"s" = "skip"
# "q"キーは終了キーと被るので設定しないでください
# "q" = "not work"
```

`shotclassif.exe`をターミナルで実行してください．

`q`キーで終了します．

移動先に同名のファイルが存在する場合は，上書きを避けるため移動されません．

## Todo

- config.tomlをコマンドラインで指定できるようにする
- カレントディレクトリを操作対象にする
- Undo機能の実装
- より高速な画像表示

## ライセンス
