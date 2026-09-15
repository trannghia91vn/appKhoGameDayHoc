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

Luồng hiện tại ưu tiên game HTML đơn file. Neu game can CSS/JS/image rieng, can mo rong updater de copy ca folder asset kem theo va giu validate path trong `read_installed_resource`.

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

Trang `Cài đặt` cho phep chon mot folder tu may hoac USB co nhieu file `.html`, vi du:

- `usb-games/toan-lop-4.html`
- `usb-games/tieng-viet-5.html`
- `usb-games/sub-folder/bai-doc.html`

Sau khi chon folder, user bam `Quet HTML`. App tao game ID tu ten file HTML, so sanh voi catalog hien tai, va chi dong bo cac file HTML moi. Khi dong bo, moi file HTML duoc chep vao `<app-data>/games/<game-id>/index.html` kem metadata `game.json` toi thieu:

```json
{
  "title": "Tên file HTML",
  "grade": "Tuy chon",
  "category": "HTML",
  "version": 1,
  "entry": "index.html"
}
```

Neu `game-id` tao tu ten file da co trong catalog, file do duoc danh dau `Da co trong kho` va khong bi chep de. Deep link van la `yeutregame://play/<game-id>`.

## Quy trinh scan va xac nhan

Khi cap nhat tu Cài đặt, launcher chi ghi vao app sau khi user bam `Xac nhan cap nhat`. Cac file HTML moi duoc chon san; file da co trong kho hien rieng va bi khoa de tranh ghi de.

Neu scan khong thay file hop le, kiem tra lai folder da chon co file `.html`/`.htm`. File khong phai HTML, file trung game ID, path khong hop le, `.DS_Store`, `Thumbs.db`, va `__MACOSX` se bi bo qua.

