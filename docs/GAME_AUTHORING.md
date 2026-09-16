# Game Authoring

Huong dan them/sua game giao duc HTML trong launcher.

## Vi tri game

Game cap nhat tu trang `Cài đặt` se duoc chep vao thu muc du lieu cua app, dang:

```text
<app-data>/games/<game-id>/index.html
```

Launcher hien khong con game mau bundle san trong catalog. Nguon game chinh la cac file HTML user dong bo tu folder/USB.

## Game ID

Game ID phai:

- Khong rong.
- Toi da 80 ky tu.
- Chi gom chu thuong `a-z`, so `0-9`, dau gach ngang `-`, va gach duoi `_`.
- On dinh lau dai vi duoc dung trong deep link PowerPoint.

Vi du tot:

```text
toan-lop-4-do-dai
tieng-viet-5-tu-dong-nghia
```

## Manifest

Moi game da cai co metadata trong `<app-data>/games/<game-id>/game.json`. Rust doc metadata nay thanh struct `GameManifest` de hien tren launcher.

Truong can giu:

- `id`: key ky thuat va deep link.
- `title`: ten hien cho giao vien.
- `grade`: lop/khoi.
- `category`: mon hoc/chu de.
- `version`: tang khi cap nhat noi dung game.
- `entry`: file mo dau, thuong la `index.html`.

## Them game don file trong app

1. Chuan bi file `.html` tren may hoac USB.
2. Vao `Cài đặt` -> `Chọn folder / USB`.
3. Bam `Quét HTML` de app preview danh sach file moi.
4. Chon cac file can them va bam `Xác nhận cập nhật`.
5. Test bang `npm run tauri:dev`, nut `Chơi`, va link `yeutregame://play/<game-id>`.

## Game nhieu file

P1 ho tro game nhieu file theo quy uoc moi game la mot folder rieng. Folder game can co `index.html`, hoac co `game.json` voi truong `entry` tro toi mot file HTML hop le trong cung folder.

Vi du:

```text
USB/
  game-a/
    index.html
    images/a.png
    data.json
  game-b/
    game.json
    play.html
    assets/card.png
```

Khi import, app chep nguyen cay file hop le vao:

```text
<app-data>/games/game-a/index.html
<app-data>/games/game-a/images/a.png
<app-data>/games/game-b/play.html
<app-data>/games/game-b/assets/card.png
```

Moi game chay doc lap qua `ytasset://game/<game-id>/<entry>`, nen asset relative nhu `images/a.png`, `assets/card.png`, CSS, JS, audio se duoc phuc vu trong dung folder game.

## Nguyen tac noi dung game

Game giao duc nen:

- Co muc tieu bai hoc ro.
- Co cau hoi/nhiem vu de giao vien dieu khien nhanh.
- Co feedback dung/sai hoac dap an giai thich.
- Co co che diem/tong ket neu phu hop lop hoc.
- Hoat dong offline, khong phu thuoc CDN.
- Khong can dang nhap.

## Thiet ke game

Tham khao `docs/DESIGN_SYSTEM.md`.

Game co the nhieu mau sac hon launcher, nhung can:

- Nut lon, text de doc tu xa.
- Layout khong scroll ngang.
- Trang thai diem/cau hoi hien ro.
- Anh/emoji/minh hoa phuc vu bai hoc, khong lam roi giao vien.

## Kiem thu game

Toi thieu:

- Mo trong app bang nut `Chơi`.
- Mo bang deep link `yeutregame://play/<game-id>`.
- Tat ca asset relative load duoc qua `ytasset`.
- Kiem tra trong khung bai tap ben phai cua app, toi thieu 860x620 khi chay desktop lon.

## Cap nhat bang folder/USB trong app

Trang `Cài đặt` cho phep chon mot hoac nhieu folder tu may/USB. App quet hai dang nguon:

- Folder game: `usb-games/game-a/index.html` hoac `usb-games/game-b/game.json` + entry HTML.
- HTML don le: `usb-games/toan-lop-4.html`, `usb-games/tieng-viet-5.html`.

Neu chon `USB/` co `game-a/` va `game-b/`, app scan ra 2 game folder rieng. Neu chon `USB/` co `game-a.html` va `game-b.html`, app scan ra 2 game HTML don rieng.

Sau khi chon folder, user bam `Quet game`. App tao game ID tu ten folder game hoac ten file HTML, so sanh voi catalog hien tai, va chi dong bo cac game moi. Khi dong bo HTML don, moi file HTML duoc chep vao `<app-data>/games/<game-id>/index.html` kem metadata `game.json` toi thieu:

```json
{
  "title": "Tên file HTML",
  "grade": "Tuy chon",
  "category": "HTML",
  "version": 1,
  "entry": "index.html"
}
```

Neu `game-id` tao tu ten folder/file da co trong catalog, game do duoc danh dau `Da co trong kho` va khong bi chep de. Deep link van la `yeutregame://play/<game-id>`.

## Quy trinh scan va xac nhan

Khi cap nhat tu Cài đặt, launcher chi ghi vao app sau khi user bam `Xac nhan cap nhat`. Cac game moi duoc chon san; game da co trong kho hien rieng va bi khoa de tranh ghi de.

Neu scan khong thay game hop le, kiem tra lai folder da chon co folder game voi entry HTML hoac file `.html`/`.htm`. File/folder trung game ID, path khong hop le, symlink, `.DS_Store`, `Thumbs.db`, va `__MACOSX` se bi bo qua.

## Backup va restore bang zip

Trang `Cài đặt` co hai thao tac cho kho game:

- `Xuất kho game`: tao file zip trong Downloads.
- `Nhập file zip`: chon file `.zip` de khoi phuc game vao app.

Cau truc zip chuan:

```text
yeutre-game-kho.zip
  categories.json           # optional
  games/
    game-a/
      index.html
      game.json             # optional
      images/a.png
    game-b/
      game.json
      play.html
      assets/card.png
```

Moi game trong zip phai nam trong `games/<game-id>/...`. `game-id` van dung chung rule cua deep link: chu thuong, so, `-`, `_`, toi da 80 ky tu. Game hop le can co `index.html`, hoac `game.json` co truong `entry` tro toi mot file HTML hop le trong folder game.

Import zip khong ghi de game da co. Neu zip co `games/game-a/...` nhung app da co `game-a`, game do bi bo qua va duoc tinh vao summary. App import vao staging tam truoc, validate entry/path/size/file count, sau do moi move vao `<app-data>/games`. Import xong app rebuild `catalog.json` de kho game hien ngay.

Zip tu app export ra la format backup chuan. Zip tao tay van dung duoc neu giu dung cau truc tren, khong co path traversal (`../`), slash nguoc, absolute path, symlink, hoac file he thong nhu `.DS_Store`.

