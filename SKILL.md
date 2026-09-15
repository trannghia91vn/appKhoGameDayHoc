# SKILL.md

Skill lam viec voi du an YeuTre Game Launcher.

## Khi nao dung skill nay

Dung file nay khi ban la AI/coding agent duoc yeu cau sua, mo rong, debug, thiet ke, them game, hoac viet tai lieu cho repo YeuTre Game Launcher.

## Cach bat dau

1. Doc `AGENTS.md` truoc.
2. Neu sua UI, doc `DESIGN.md` va `docs/DESIGN_SYSTEM.md`.
3. Neu sua deep link, command Tauri, protocol asset, hoac player window, doc `docs/ARCHITECTURE.md` va `docs/DEEP_LINKS.md`.
4. Neu them/sua game HTML, doc `docs/GAME_AUTHORING.md`.
5. Doc file code lien quan sau khi doc tai lieu, khong sua theo gia dinh.

## Mo hinh tinh than can giu

Day la desktop launcher cho giao vien mo game giao duc HTML tu kho local, dac biet qua deep link tu PowerPoint. Launcher khong phai website marketing. Player window la noi game chay. Rust la lop bao ve duong dan, game ID, va tai nguyen.

## Viec nen lam

- Giu launcher don gian ve workflow nhung dung visual rules Discord-inspired trong `DESIGN.md`: indigo canvas, Blurple, green CTA, magenta accents, rounded panels.
- Giu thiet ke launcher sang, gon, lam viec tot tren desktop 1100x760 va mobile/narrow viewport.
- Giu game ID dang `a-z`, `0-9`, `-`, `_`, toi da 80 ky tu.
- Giu game local/offline-first; game runtime duoc cap nhat vao app data qua tab `Cài đặt`.
- Giu luong account hien tai: User khong can password, Admin dung password hash trong `localStorage`; doi password trong `Cài đặt` khong yeu cau password cu.
- Cap nhat catalog khi them game moi.
- Them/cap nhat test Rust khi dong vao parser/protocol validation.
- Chay `npm run build` va `cd src-tauri && cargo test` khi co thay doi lien quan.

## Viec can tranh

- Khong dua game logic vao React launcher.
- Khong bo validate trong `deep_link/parser.rs` hoac `protocol/game_protocol.rs`.
- Khong cho asset path doc truc tiep filesystem tu input user.
- Khong them CDN/remote script vao game neu khong duoc yeu cau.
- Khong bien launcher thanh landing page dai dong; hero/visual energy phai phuc vu kho game va thao tac cua giao vien.
- Khong doi scheme `yeutregame` hoac protocol `ytasset` neu khong co migration ro rang.

## Checklist truoc khi ket thuc task

- Luong `Chơi` van mo dung player window.
- Link dang `yeutregame://play/<game-id>` van duoc parse dung.
- Asset game van duoc load qua `ytasset://game/<game-id>/<entry>`.
- CSP trong `src-tauri/tauri.conf.json` van phu hop voi cach load asset.
- UI khong tran chu trong nut/card/status row.
- Tai lieu duoc cap nhat neu thay doi kien truc, scheme, catalog, hoac design pattern.
