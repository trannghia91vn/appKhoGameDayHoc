# Architecture

YeuTre Game Launcher tach ro 3 lop: launcher React, runtime Tauri/Rust, va game HTML local.

## Lop 1: React launcher

File chinh: `src/main.tsx`.

Trach nhiem:

- Goi `list_games` khi app load.
- Co topbar `Kho game` / `Cài đặt`; `Kho game` la trang catalog, `Cài đặt` la noi cap nhat source games.
- Hien danh sach game da cai.
- Cho phep chon nhieu game da cai va xoa hang loat sau khi user xac nhan.
- Load `ytasset://game/<game-id>/<entry>` vao iframe ben phai khi user bam `Chơi`.
- Nghe event `deep-link-status` tu Rust de hien trang thai deep link PowerPoint.
- Hien status/error bang tieng Viet.

React khong doc file game, khong parse filesystem path; URL asset iframe chi duoc tao tu manifest game da duoc backend tra ve.

## Lop 2: Tauri/Rust trusted runtime

File chinh: `src-tauri/src/lib.rs`.

Dang ky:

- `tauri_plugin_deep_link` de nhan scheme `yeutregame`.
- `tauri_plugin_single_instance` de app dang chay nhan argv tu instance moi.
- `tauri_plugin_log` de ghi log.
- Protocol bat dong bo `ytasset`.
- Commands `last_deep_link_status`, `list_games`, `open_deep_link`, `delete_games`, `scan_games_from_files`, `install_games_from_files`, `list_categories`, `save_category`, `delete_category`, `classify_games_by_categories`; deep link tu OS duoc Rust xu ly truc tiep qua `handle_deep_link`.

Commands:

- `list_games() -> Vec<GameManifest>`: tra games da cai cho launcher.
- `last_deep_link_status(app)`: tra status deep link gan nhat de frontend replay loi/trang thai neu app cold-start tu PowerPoint truoc khi React listen.
- `open_deep_link(app, url)`: di qua `handle_deep_link`, parse URL, focus launcher, chon game trong `Kho game`, log ket qua va emit `deep-link-status`; frontend sau do load game vao iframe ben phai khi game list da san sang.
- `scan_games_from_files(app, files)`: quet cac file `.html` trong folder/USB, tao game ID tu ten file, va tra danh sach file moi/da co de frontend review.
- `install_games_from_files(app, files, gameIds)`: chi sync cac file HTML moi da duoc user xac nhan vao `<app-data>/games/<game-id>/index.html` kem `game.json` toi thieu.
- `delete_games(app, gameIds)`: chi xoa thu muc game da cai trong app data; khong xoa game bundle.
- `classify_games_by_categories(app)`: doc categories runtime, doi keywords va ten game ve chu khong dau, sau do cap nhat `category` trong `game.json` cua game da cai neu ten file/title khop keyword.

## Lop 3: Embedded exercise view va asset protocol

React hien khung bai tap ben phai voi ty le 7/10 man hinh. Khi user bam `Choi ngay` hoac deep link hop le duoc nhan, iframe trong app load `ytasset://game/<game-id>/<entry>`; app khong tao cua so player rieng.

`src-tauri/src/protocol/game_protocol.rs` xu ly request asset:

- Chi chap nhan host `game`.
- Decode path percent-encoding.
- Tach thanh `<game-id>/<resource-path>`.
- Validate game ID bang parser chung.
- Validate resource path chong traversal.
- Lay bytes tu `games::catalog::read_resource`.
- Tra MIME bang `mime_guess`.

## Catalog hien tai

File: `src-tauri/src/games/catalog.rs`.

Catalog hien nay chi gom games da cai trong app data (`appData/games/<game-id>`). Game da cai co the co `game.json`; neu khong co, app dung title suy ra tu folder, category `Game`, grade `Tuy chon`, version `1`, entry `index.html`. Moi manifest co `isInstalled = true` vi app khong con game bundle mau trong catalog.

## Bao mat va tin cay

Cac guard quan trong:

- Deep link khong duoc co `..`, slash nguoc, query, fragment.
- Deep link chi ho tro `yeutregame://play/<game-id>`.
- Game ID chi gom `a-z`, `0-9`, `-`, `_`, toi da 80 ky tu.
- Asset protocol chi phuc vu host `game`.
- Resource path khong duoc rong, bat dau bang `/`, chua slash nguoc, segment `.`, `..`, hoac segment rong.
- CSP trong `tauri.conf.json` cho phep app self, IPC, asset image, inline style; khong mo rong neu khong can.

## Huong mo rong sau PoC

Neu catalog lon hon:

- Uu tien mot manifest co cau truc, vi du JSON/TOML build-time hoac runtime local database.
- Van giu mot ham lookup tap trung cho manifest.
- Van giu protocol handler la noi validate cuoi cung truoc khi tra bytes.
- Neu game co nhieu asset, can mapping resource ro rang thay vi noi chuoi path doc filesystem truc tiep tu input.

## Cap nhat games tu Cài đặt

Frontend dung input directory picker de doc toan bo file trong folder/USB. Cac file duoc giu trong state tam thoi va gui cho Rust command scan_games_from_files de preview; frontend khong tu quyet dinh game nao hop le.

Quy trinh bat buoc:

1. User chon folder/USB.
2. Frontend giu folder da chon; khi user bam `Quet HTML`, frontend doc file bytes va relativePath, sau do goi scan_games_from_files.
3. Rust normalize path, bo qua .DS_Store/Thumbs.db/__MACOSX, chi nhan `.html`/`.htm`, tao game ID tu ten file, va so sanh voi catalog hien tai.
4. Rust tra danh sach DiscoveredGame, trong do isNew cho biet game chua co trong catalog.
5. Frontend hien danh sach checkbox; file moi duoc chon mac dinh, file da co trong kho bi khoa de tranh ghi de.
6. User bam Xac nhan cap nhat. Frontend gui toan bo source bytes kem gameIds moi da chon cho install_games_from_files.
7. Rust chi ghi cac game ID moi duoc chon; file da co trong catalog khong bi ghi de.

Backend khong tin path tu frontend: no normalize segment, tao/validate game ID tu ten file HTML, bo qua file he thong, va khong cho ghi ra ngoai app data. Khi game ID da co trong catalog, backend bo qua de tranh ghi de. Lenh delete_games validate tung game ID va chi thao tac trong app data/games.

## Categories trong Cài đặt

Frontend cung cap form tao/sua category voi hai truong `name` va `keywords`. Keywords duoc nhap tren UI bang chuoi phan tach dau phay, sau do gui cho Rust duoi dang mang chuoi da trim va loai trung.

Backend luu danh sach tai `<app-data>/categories.json` qua cac commands:

- `list_categories()`: doc danh sach category runtime.
- `save_category(input)`: tao moi hoac cap nhat category theo ID on dinh tao tu ten.
- `delete_category(categoryId)`: xoa mot category sau khi frontend xac nhan.

Category CRUD doc lap voi game catalog: xoa category khong xoa file game. Ten category khong duoc trung, keywords phai co it nhat mot gia tri, va ID duoc validate/normalize de dung on dinh cho cac lan cap nhat sau.

Section `Phan loai game` dung command `classify_games_by_categories()` de gan category cho game HTML da cai. Backend chi xu ly game co `isInstalled = true`. Thuat toan tao search text tu `title + id`, normalize ve lowercase ASCII khong dau, thay dau/cac ky tu dac biet bang khoang trang, roi so tung keyword da normalize theo whole-token substring. Neu nhieu category khop, category co so keyword khop nhieu nhat duoc chon; neu bang nhau thi giu category xuat hien truoc trong danh sach da luu. Khi category thay doi, backend ghi lai `<app-data>/games/<game-id>/game.json` va giu cac field metadata khac.

## UI Design Contract

App shell phai ap dung rules tu `DESIGN.md` va `docs/DESIGN_SYSTEM.md`: topbar toi, active tab nen trang, hero Blurple/Magenta, game cards surface indigo, nut chinh green CTA. Kien truc React/Tauri khong thay doi khi doi visual style: React chi dieu huong/hien UI va goi command, Rust van xu ly catalog, deep link, protocol va sync games.
