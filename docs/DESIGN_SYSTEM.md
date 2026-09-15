# Design System

Tai lieu nay la ban dien giai ap dung cho app tu `DESIGN.md`. `DESIGN.md` la source rules goc, con file nay noi ro cach dung rules do trong YeuTre Game Launcher.

## Huong thiet ke chinh

App su dung ngon ngu thiet ke lay cam hung Discord: gaming-native, nang luong cao, indigo sau, Blurple noi bat, xanh dien cho hanh dong chinh, magenta cho cac mang gradient vui. Cam giac can gan voi kho game/presentation launcher hon la cong cu hanh chinh kho kho.

Tinh cach:

- Vui, dam, arcade, phu hop game giao duc.
- Van ro rang cho giao vien: luong chinh `Kho game` va `Cài đặt` phai quet mat nhanh.
- Khong corporate, khong dashboard xam trang, khong marketing landing page dai dong.

## Token tu DESIGN.md

Mau cot loi:

- Primary Blurple: `#5865f2`.
- Green CTA: `#35ed7e`.
- Magenta accent: `#ec48bd`.
- Link/electric blue: `#00b0f4`.
- Canvas: `#0a0d3a`.
- Surface indigo: `#1e2353`.
- Surface black: `#000000`.
- Ink: `#ffffff`.
- Hairline: `#23272a` hoac bien the trong suot tren nen toi.

Bo tron:

- Nut/tab nho: 12-16px.
- Card/panel: 24px trong app hien tai, co the len 40px cho hero/feature band.
- Pill/full: 9999px khi can badge/pill.

Typography:

- Uu tien stack `ABC Ginto Nord`, `ABC Ginto`, `gg sans`, roi fallback system.
- Display/heading lon, dam, line-height chat, letter-spacing bang `0`.
- Hero co the uppercase de tao cam giac gaming Discord.
- Body text van de doc, khoang 16-20px, line-height 1.4-1.56.

## App Shell

`body` va app nen la deep-indigo canvas, co the dung radial gradients nhe de tao energy. Duoc phep dung grid texture rat nhe neu khong lam roi mat.

`app-shell`:

- Max width khoang 1180px.
- Padding desktop 24px+.
- Mobile 14-16px.

## Topbar

Topbar la menu chinh voi 2 option dau tien:

- `Kho game`: trang catalog de load game va mo game.
- `Cài đặt`: trang thiet lap/cap nhat source games.

Style:

- Nen canvas trong suot/dam.
- Border hairline trang trong suot.
- Bo tron lon 20-24px.
- Tab active nen trang, text den de tao contrast Discord-style.
- Tab inactive nen trong suot, hover surface indigo.

## Kho Game

Hero:

- Dung gradient Blurple -> Magenta.
- Text trang, heading rat dam, uppercase.
- Bo tron lon 40px desktop, giam nhe tren mobile.
- Protocol box la surface toi nam trong hero, border hairline.

Status row:

- Nen den/surface-black.
- Text trang.
- Loi hoac diem nhan dung green/electric accent.
- Khong dung modal cho loi nho.

Game cards:

- Surface indigo, border hairline, shadow sau.
- Co the xen gradient Blurple/Magenta theo pattern nhe.
- Nut `Chơi` la green CTA, text den.
- Game ID/code chip nam tren nen toi trong suot.

## Kho Game: Xóa Hàng Loạt

Danh sach game dung checkbox rieng cho game da cai; checkbox khong duoc lam can tro thao tac click de mo chi tiet game.

- Thanh thao tac dat gan danh sach: Chon tat ca dang hien thi, dem so game da chon, va nut Xoa da chon.
- Nut xoa dung mau danger do, contrast cao, va luon yeu cau xac nhan truoc khi xoa.
- Game bundle mau hien checkbox disabled vi khong nam trong app data.
- Sau khi xoa, catalog va game dang focus phai duoc refresh; khong de lai selection ID da bi xoa.

## Cài Đặt

Trang `Cài đặt` cung dung surface indigo/panel toi, khong quay lai palette xam trang.

Muc `Categories`:

- Form nam tren danh sach, gom `Tên Category` va `Keywords`.
- Keywords hien thi dang chip sau khi luu, input dung dau phay de tach nhieu gia tri.
- Moi item co nut `Sửa` de nap lai form va nut `Xóa` voi confirmation modal rieng.
- Category config nam trong panel indigo cung he thong voi muc cap nhat games.

Muc `Cập nhật games`:

- Nut `Chọn folder / USB` dung white button.
- Nut `Cập Nhật` dung green CTA.
- Source da chon nam trong surface den trong suot.
- Summary thanh cong dung gradient green/electric blue, text den.

Muc `Phân loại game`:

- Dat sau `Categories` de user tao/sua keywords truoc khi chay phan loai.
- Nut `Phân loại game` dung green CTA, disabled neu chua co category hoac chua co game da cai.
- Status hien so categories va so game da cai tren surface den trong suot.
- Ket qua hien stat cards ngan gon: da quet, khop keywords, da cap nhat, chua khop; danh sach ket qua co scroll neu dai.

## Game HTML trong Player

Game ben trong player co the vui hon launcher va co style rieng, nhung nen ton trong mood tong the:

- Mau sac manh, nut lon, feedback ro.
- Khong bat buoc dung Blurple/Magenta neu bai hoc can chu de rieng.
- Van nen offline-first va de doc tren man hinh lop hoc.

## Responsive

- Tu 900px tro xuong, hero ve 1 cot va giam font hero.
- Tu 760px tro xuong, topbar/folder-picker/section heading ve 1 cot.
- Text khong tran nut/card/status.
- Khong scale font bang viewport width; dung breakpoint ro rang.

## Nhung dieu can tranh

- Khong quay ve design xam trang utility neu task lien quan app shell.
- Khong dung official Discord/Microsoft/PowerPoint logos trong UI neu khong co license ro.
- Khong lam mat contrast tren nen gradient.
- Khong them qua nhieu decoration lam mo luong `Chơi` va `Cập Nhật`.
- Khong dua remote font/CDN vao app neu chua co ly do; font stack phai co fallback tot.
- Khong lam UI phu thuoc internet.
