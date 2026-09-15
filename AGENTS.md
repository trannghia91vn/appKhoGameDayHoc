# AGENTS.md

Huong dan nhanh cho AI/coding agent tiep tuc du an YeuTre Game Launcher.

## Muc tieu san pham

YeuTre Game Launcher la app desktop Tauri dung de mo kho game giao duc HTML cuc bo. App hien tai la PoC nhung kien truc can duoc giu on dinh:

- Man hinh chinh React la launcher: co topbar `Kho game` / `Cài đặt`, hien catalog game, trang thai, loi, nut choi, va vi du deep link.
- Dang nhap co 2 role: `User` vao khong can password; `Admin` dung password hash luu o `localStorage`, co the doi trong `Cài đặt` ma khong can nhap password cu.
- Rust/Tauri la lop tin cay: parse deep link, validate game ID/path, phuc vu file game qua custom protocol de React nhung vao khung bai tap trong app.
- PowerPoint hoac tai lieu ben ngoai co the goi game bang link `yeutregame://play/<game-id>`.

## Stack hien tai

- Frontend: React 18, TypeScript, Vite.
- Desktop shell: Tauri 2.
- Deep link: `@tauri-apps/plugin-deep-link` va `tauri-plugin-single-instance`.
- Asset protocol: `ytasset://game/<game-id>/<resource-path>`.

## Lenh thuong dung

Chay web dev server:

```bash
npm run dev
```

Chay app Tauri:

```bash
npm run tauri:dev
```

Build frontend:

```bash
npm run build
```

Build desktop bundle:

```bash
npm run tauri:build
```

Chay test Rust:

```bash
cd src-tauri
cargo test
```

## Ban do file quan trong

- `src/main.tsx`: React launcher UI, goi command Tauri `list_games`, nghe deep link, va nhung bai tap vao iframe `ytasset` ben trong app.
- `src/styles.css`: design system hien tai cua launcher.
- `src-tauri/src/lib.rs`: dang ky plugin, command, single-instance forwarding, va protocol `ytasset`.
- `src-tauri/src/deep_link/parser.rs`: parse/validate `yeutregame://play/<game-id>`.
- `src-tauri/src/protocol/game_protocol.rs`: validate asset path va tra bytes game.
- `src-tauri/src/games/catalog.rs`: catalog game bundle + game da cai trong app data.
- `src-tauri/src/games/install.rs`: nhan file tu folder picker frontend va sync source games vao app data.
- `src-tauri/tauri.conf.json`: window, CSP, bundle resources, scheme deep link.

## Nguyen tac thiet ke chinh

Doc them `docs/DESIGN_SYSTEM.md` truoc khi sua UI.

Launcher can di theo rules trong `DESIGN.md`: Discord-inspired, gaming-native, deep indigo canvas, Blurple, green CTA, magenta gradients, bo tron lon va typography dam. Van giu luong giao vien ro rang: `Kho game` de mo game, `Cài đặt` de cap nhat/thiet lap games.

## Luong chinh can bao toan

1. User bam `Chơi` hoac mo link `yeutregame://play/<game-id>` cua game da cai.
2. Nut `Chơi` tu React set bai tap dang chay va load `ytasset://game/<game-id>/<entry>` vao khung view ben phai; khong mo cua so rieng.
3. Rust validate `game_id`.
4. Rust tim manifest trong catalog.
5. Deep link: Rust focus cua so launcher va emit status de React chon game trong `Kho game`; React load game do vao khung view ben phai.
6. Iframe bai tap nap `ytasset://game/<game-id>/<entry>`.
7. Protocol handler validate host/path, doc bytes tu catalog, tra dung MIME.
8. Rust emit `deep-link-status` de React hien trang thai/lỗi cho user va trigger khung bai tap khi game ID khop.

Khong bo qua buoc validate. Khong cho phep `..`, slash nguoc, query/fragment trong deep link, hoac resource path bat dau bang `/`.

## Khi them game moi

Doc `docs/GAME_AUTHORING.md`.

Catalog hien tai chi doc game da cap nhat vao app data qua trang `Cài đặt`. Source HTML chon tu folder/USB duoc quet sau khi user bam `Quet HTML`; moi file `.html` hop le duoc xem nhu mot game don file. App so sanh game ID tao tu ten file voi catalog hien tai, chi file HTML moi duoc chon de dong bo khi user bam `Xac nhan cap nhat`.

Trang `Cài đặt` cung co CRUD Categories runtime: form nhap ten category va keywords phan tach dau phay, list ben duoi co sua/xoa, du lieu luu tai app data qua cac command `list_categories`, `save_category`, `delete_category`. Category la cau hinh doc lap, khong phai game catalog; keywords duoc dung boi section `Phan loai game`.

Khi user bam `Phan loai game`, command `classify_games_by_categories` quet game da cai trong app data, normalize tieng Viet ve khong dau cho ca keywords va ten/title game, roi ghi lai `category` trong tung `game.json` neu khop. Khong sua game bundle mau va khong xoa file game. Neu sua logic nay, cap nhat Rust tests trong `src-tauri/src/games/install.rs`.

## Quy tac sua code

- Giu TypeScript typed ro rang, tranh bien React launcher thanh noi chua logic game.
- Loi hien thi cho user nen bang tieng Viet co dau khi sua noi dung UI moi.
- Loi Rust hien tai co nhieu chuoi khong dau; neu chinh sua khu vuc do, co the chuan hoa dan nhung khong refactor lan rong neu khong can.
- Khong dua remote asset/CDN vao game hay launcher neu chua co ly do ro. App can uu tien chay offline.
- Khi sua deep link/protocol/khung bai tap nhung trong app, them hoac cap nhat test Rust neu co thay doi backend.
- Khi sua layout launcher, ap dung `DESIGN.md` va `docs/DESIGN_SYSTEM.md`; test o chieu rong desktop va mobile nho; text khong duoc tran nut/card.

## Tai lieu bo sung

- `SKILL.md`: playbook ngan cho AI khi bat dau mot task trong repo.
- `docs/ARCHITECTURE.md`: kien truc va contract giua React/Tauri.
- `docs/DESIGN_SYSTEM.md`: huong dan thiet ke launcher va game.
- `docs/DEEP_LINKS.md`: scheme, validation, event forwarding.
- `docs/GAME_AUTHORING.md`: cach them/sua game HTML.
