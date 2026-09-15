# Deep Links

Deep link la contract quan trong giua PowerPoint/tai lieu ngoai va desktop launcher.

## Scheme

Dinh dang hop le duy nhat hien tai:

```text
yeutregame://play/<game-id>
```

Vi du:

```text
yeutregame://play/toan-lop-4
```

## Validation

File: `src-tauri/src/deep_link/parser.rs`.

Parser hien tai:

- Tu choi input chua `..`.
- Tu choi slash nguoc `\`.
- Yeu cau scheme la `yeutregame`.
- Cho phep query do PowerPoint tu them, vi du `?OR=PowerPoint`, va bo qua query nay; van tu choi fragment.
- Yeu cau host/command la `play`.
- Yeu cau dung mot path segment la game ID.
- Validate game ID khong rong, toi da 80 ky tu, chi gom `a-z`, `0-9`, `-`, `_`.

Khong dung query de dieu khien app; query hien chi duoc bo qua de tuong thich PowerPoint/macOS.

## Luong khi app dang chay

Rust la noi xu ly deep link chinh:

- `handle_deep_link(app, url, source)` parse URL, validate game ID, focus cua so launcher va emit status `selected` cho game trong `Kho game`. Frontend se goi logic `Chơi ngay` sau khi game list da load va ID khop.
- `open_deep_link` command, single-instance callback, startup `get_current()`, va runtime `on_open_url()` deu di qua helper nay.
- Rust log `deep_link_received`, `deep_link_selected`, hoac `deep_link_failed`.
- Rust emit `deep-link-status` de frontend hien trang thai/link/game/lỗi cho user. Status gan nhat duoc luu trong state va doc lai qua `last_deep_link_status` de khong mat loi khi cold-start.

Frontend nghe `deep-link-status`, focus game trong `Kho game` khi Rust bao `selected`, roi load game vao iframe ben phai sau khi game list san sang.

## Chong mo trung

Neu cung mot link duoc bam nhieu lan, app focus lai launcher, chon game tuong ung va load lai khung bai tap. Frontend co guard ngan de tranh replay cung link qua nhanh.

## Khi loi

Neu parser hoac load bai tap loi:

- Rust log `deep_link_failed`.
- Rust emit `deep-link-status` voi `status = failed` va message cu the.
- Frontend hien status `Khong the mo game tu link PowerPoint.` kem noi dung loi, vi du `Khong tim thay tro choi co ma: <game-id>`.

Thong diep loi moi nen:

- Ro nguyen nhan.
- Than thien voi giao vien/nguoi dung cuoi neu hien len UI.
- Khong lo filesystem path nhay cam.

## Neu doi contract

Khi thay doi scheme/command:

- Cap nhat `src-tauri/tauri.conf.json`.
- Cap nhat `parser.rs` va test.
- Cap nhat vi du trong `src/main.tsx`.
- Cap nhat tai lieu nay va `AGENTS.md`.
- Can ke hoach migration cho cac file PowerPoint da gan link cu.
