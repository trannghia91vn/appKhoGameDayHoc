import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

const APP_NAME = "Kho game cô Trang Trần";
const APP_INITIALS = "CT";
const APP_VERSION = import.meta.env.PACKAGE_VERSION;

type Game = {
  id: string;
  title: string;
  grade: string;
  category: string;
  version: number;
  entry: string;
  isInstalled?: boolean;
};

type InstallGameFile = {
  relativePath: string;
  bytes: number[];
};

type InstallGamePath = {
  relativePath: string;
  path: string;
};

type InstallGamesSummary = {
  copiedFiles: number;
  installedGames: number;
  skippedFiles: number;
  targetDir: string;
  gameIds: string[];
};

type ExportGamesArchiveSummary = {
  archivePath: string;
  exportedGames: number;
  exportedFiles: number;
  archiveBytes: number;
};

type ImportGamesArchiveSummary = {
  archivePath: string;
  importedGames: number;
  importedFiles: number;
  skippedGames: number;
  skippedFiles: number;
  gameIds: string[];
};

type AppDiagnostics = {
  appDataDir: string;
  gamesDir: string;
  installedGames: number;
  totalGameBytes: number;
  catalogCacheExists: boolean;
  catalogCacheValid: boolean;
  categoriesCount: number;
};

type ExportGamesArchiveProgress = {
  exportedFiles: number;
  currentPath: string;
};

type DeleteGamesSummary = {
  deletedGames: number;
  skippedGames: number;
  gameIds: string[];
};

type ClassifiedGame = {
  id: string;
  title: string;
  previousCategory: string;
  category: string;
  matchedKeywords: string[];
  updated: boolean;
};

type ClassifyGamesSummary = {
  scannedGames: number;
  matchedGames: number;
  updatedGames: number;
  unchangedGames: number;
  skippedGames: number;
  games: ClassifiedGame[];
};

type Category = {
  id: string;
  name: string;
  keywords: string[];
};

type PendingCategoryDelete = Category;

type GameScanCandidate = Game & {
  fileCount: number;
  isNew: boolean;
  sourcePath: string;
  sourceKind: "folder" | "html";
  totalBytes: number;
};

type ScanGamesSummary = {
  games: GameScanCandidate[];
  skippedFiles: number;
};

type Page = "library" | "settings";
type AccountRole = "admin" | "user";

type DebugEntry = {
  at: string;
  message: string;
};

type PendingDelete = {
  gameIds: string[];
  titles: string;
};

type DeepLinkStatus = {
  status: "received" | "selected" | "failed";
  source: string;
  url: string;
  gameId?: string;
  message: string;
};

type GameRuntimeMessage = {
  source: "yeutre-game-runtime";
  level: "ready" | "error" | "unhandledrejection" | "securitypolicyviolation" | "storage-fallback" | "drag-drop";
  message: string;
  filename?: string;
  line?: number;
  column?: number;
  blockedURI?: string;
  violatedDirective?: string;
  href?: string;
  readyState?: string;
  key?: string;
  bytes?: number;
};

const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const ADMIN_PASSWORD_STORAGE_KEY = "yeutre.gameLauncher.adminPasswordHash.v1";
const ADMIN_PASSWORD_SALT = "yeutre-game-launcher-admin-v1";
const MAX_BROWSER_SCAN_FILES = 2_000;
const MAX_BROWSER_HTML_FILE_BYTES = 80 * 1024 * 1024;
const VIRTUAL_GAME_LIST_THRESHOLD = 150;
const VIRTUAL_GAME_ROW_HEIGHT = 75;
const VIRTUAL_GAME_LIST_OVERSCAN = 8;

type TauriInputFile = File & {
  path?: string;
};

function fileRelativePath(file: File) {
  return file.webkitRelativePath || file.name;
}

function fileSystemPath(file: File) {
  return (file as TauriInputFile).path?.trim() || null;
}

function htmlSourcePathFiles(files: File[]) {
  const htmlFiles = files.filter((file) => isHtmlFileName(file.name));
  const sourceFiles = htmlFiles
    .map((file) => {
      const path = fileSystemPath(file);
      return path ? { relativePath: fileRelativePath(file), path } : null;
    })
    .filter((file): file is InstallGamePath => file !== null);

  return sourceFiles.length === htmlFiles.length ? sourceFiles : [];
}

function displayNameForSourcePath(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || path;
}

function waitForUiFrame() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => resolve());
  });
}

type LoginScreenProps = {
  onLogin: (role: AccountRole) => void;
};

type LauncherAppProps = {
  accountRole: AccountRole;
  onLogout: () => void;
};

async function hashAdminPassword(password: string) {
  const input = ADMIN_PASSWORD_SALT + "::" + password;

  if (window.crypto?.subtle) {
    const digest = await window.crypto.subtle.digest("SHA-256", new TextEncoder().encode(input));
    return Array.from(new Uint8Array(digest))
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join("");
  }

  return "plain:" + encodeURIComponent(input);
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const formatted = value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1);
  return formatted + " " + units[unitIndex];
}

function errorMessage(error: unknown) {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  try {
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
}

function debugValue(value: unknown) {
  if (value instanceof Error) {
    return { name: value.name, message: value.message, stack: value.stack };
  }

  try {
    return JSON.parse(JSON.stringify(value));
  } catch {
    return String(value);
  }
}

function gameIdFromFileName(fileName: string) {
  const stem = fileName.replace(/\.[^/.]+$/, "");
  const withoutTone = stem
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/đ/g, "d")
    .replace(/Đ/g, "D");
  const id = withoutTone
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80)
    .replace(/-+$/g, "");

  return id || null;
}

function titleFromFileName(fileName: string) {
  return fileName.replace(/\.[^/.]+$/, "").replace(/[-_]+/g, " ").trim() || fileName;
}

function normalizeSearchText(value: string) {
  const normalized = value
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/đ/g, "d")
    .replace(/Đ/g, "D")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();

  return normalized || null;
}

function containsNormalizedKeyword(searchText: string, keyword: string) {
  return (" " + searchText + " ").includes(" " + keyword + " ");
}

function classifyPreviewGames(currentGames: Game[], currentCategories: Category[]) {
  if (currentCategories.length === 0) {
    throw new Error("Hãy tạo ít nhất một category trước khi phân loại game.");
  }

  const categoryRules = currentCategories
    .map((category) => ({
      category,
      keywords: category.keywords
        .map((keyword) => ({ keyword, normalized: normalizeSearchText(keyword) }))
        .filter((item): item is { keyword: string; normalized: string } => Boolean(item.normalized)),
    }))
    .filter((rule) => rule.keywords.length > 0);

  if (categoryRules.length === 0) {
    throw new Error("Các category hiện tại chưa có keyword hợp lệ để phân loại.");
  }

  const installedGames = currentGames.filter((game) => game.isInstalled !== false);
  if (installedGames.length === 0) {
    throw new Error("Chưa có game đã cài nào để phân loại.");
  }

  const classifiedGames: ClassifiedGame[] = [];
  let skippedGames = 0;
  const nextGames = currentGames.map((game) => {
    if (game.isInstalled === false) {
      return game;
    }

    const searchText = normalizeSearchText(game.title + " " + game.id);
    if (!searchText) {
      skippedGames += 1;
      return game;
    }

    let bestMatch: { category: Category; matchedKeywords: string[] } | null = null;
    for (const rule of categoryRules) {
      const matchedKeywords = rule.keywords
        .filter((item) => containsNormalizedKeyword(searchText, item.normalized))
        .map((item) => item.keyword);
      if (matchedKeywords.length === 0) {
        continue;
      }
      if (!bestMatch || matchedKeywords.length > bestMatch.matchedKeywords.length) {
        bestMatch = { category: rule.category, matchedKeywords };
      }
    }

    if (!bestMatch) {
      skippedGames += 1;
      return game;
    }

    const updated = game.category !== bestMatch.category.name;
    classifiedGames.push({
      id: game.id,
      title: game.title,
      previousCategory: game.category,
      category: bestMatch.category.name,
      matchedKeywords: bestMatch.matchedKeywords,
      updated,
    });

    return updated ? { ...game, category: bestMatch.category.name } : game;
  });

  const updatedGames = classifiedGames.filter((game) => game.updated).length;
  return {
    nextGames,
    summary: {
      scannedGames: classifiedGames.length + skippedGames,
      matchedGames: classifiedGames.length,
      updatedGames,
      unchangedGames: classifiedGames.length - updatedGames,
      skippedGames,
      games: classifiedGames,
    },
  };
}

function isHtmlFileName(fileName: string) {
  return /\.html?$/i.test(fileName);
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function gameAssetUrl(game: Game) {
  return convertFileSrc(`game/${game.id}/${game.entry}`, "ytasset");
}

function buildPreviewPlayerHtml(game: Game) {
  const title = escapeHtml(game.title);
  const meta = escapeHtml(`${game.category} · ${game.grade} · ${game.id}`);

  return `<!doctype html>
<html lang="vi">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <style>
    body {
      align-items: center;
      background: linear-gradient(135deg, #5865f2, #ec48bd);
      color: white;
      display: grid;
      font-family: Inter, system-ui, sans-serif;
      min-height: 100vh;
      margin: 0;
      padding: 32px;
      text-align: center;
    }
    main { display: grid; gap: 16px; }
    h1 { font-size: clamp(2rem, 6vw, 5rem); line-height: 1; margin: 0; text-transform: uppercase; }
    p { font-size: 1.15rem; font-weight: 800; margin: 0; opacity: .92; }
  </style>
</head>
<body>
  <main>
    <p>Preview bài tập</p>
    <h1>${title}</h1>
    <p>${meta}</p>
  </main>
</body>
</html>`;
}

function buildPreviewScanResult(files: File[], installedGames: Game[]): ScanGamesSummary {
  const installedIds = new Set(installedGames.map((game) => game.id));
  const seenIds = new Set<string>();
  const games: GameScanCandidate[] = [];
  let skippedFiles = 0;

  for (const file of files) {
    const sourcePath = file.webkitRelativePath || file.name;
    if (!isHtmlFileName(file.name)) {
      skippedFiles += 1;
      continue;
    }

    const id = gameIdFromFileName(file.name);
    if (!id || seenIds.has(id)) {
      skippedFiles += 1;
      continue;
    }

    seenIds.add(id);
    games.push({
      id,
      title: titleFromFileName(file.name),
      grade: "Tùy chọn",
      category: "HTML",
      version: 1,
      entry: "index.html",
      fileCount: 1,
      isNew: !installedIds.has(id),
      sourcePath,
      sourceKind: "html",
      totalBytes: file.size,
    });
  }

  if (games.length === 0) {
    throw new Error("Không tìm thấy file .html hợp lệ trong folder đã chọn.");
  }

  return { games, skippedFiles };
}

const previewGames: Game[] = [
  {
    id: "word-race",
    title: "Đua chữ tiếng Việt",
    grade: "Lớp 2",
    category: "Tiếng Việt",
    version: 1,
    entry: "index.html",
  },
  {
    id: "memory-cards",
    title: "Lật thẻ ghi nhớ",
    grade: "Mầm non",
    category: "Tư duy",
    version: 2,
    entry: "index.html",
  },
];

function LoginScreen({ onLogin }: LoginScreenProps) {
  const [role, setRole] = useState<AccountRole>("user");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [isSetup, setIsSetup] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    setIsSetup(!window.localStorage.getItem(ADMIN_PASSWORD_STORAGE_KEY));
  }, []);

  const needsPassword = isSetup || role === "admin";
  const canSubmit = isSetup
    ? password.trim().length >= 4 && password === confirmPassword
    : role === "user" || password.trim().length > 0;

  const submitLogin = useCallback(async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setAuthError(null);

    try {
      setIsSubmitting(true);

      if (isSetup) {
        if (password.trim().length < 4) {
          setAuthError("Password admin cần ít nhất 4 ký tự.");
          return;
        }

        if (password !== confirmPassword) {
          setAuthError("Hai lần nhập password admin chưa khớp.");
          return;
        }

        window.localStorage.setItem(ADMIN_PASSWORD_STORAGE_KEY, await hashAdminPassword(password));
        onLogin("admin");
        return;
      }

      if (role === "user") {
        onLogin("user");
        return;
      }

      const savedHash = window.localStorage.getItem(ADMIN_PASSWORD_STORAGE_KEY);
      if (!savedHash) {
        setIsSetup(true);
        setAuthError("Chưa thiết lập password admin.");
        return;
      }

      if (await hashAdminPassword(password) !== savedHash) {
        setAuthError("Password admin không đúng.");
        return;
      }

      onLogin("admin");
    } catch (err) {
      setAuthError(errorMessage(err));
    } finally {
      setIsSubmitting(false);
    }
  }, [confirmPassword, isSetup, onLogin, password, role]);

  return (
    <main className="login-shell">
      <form className="login-card" onSubmit={(event) => void submitLogin(event)}>
        <div className="login-brand">
          <span>{APP_INITIALS}</span>
          <div>
            <div className="brand-title-line">
              <strong>{APP_NAME}</strong>
              <span className="version-badge">v{APP_VERSION}</span>
            </div>
            <h1>{isSetup ? "Thiết lập admin" : "Đăng nhập"}</h1>
          </div>
        </div>

        {!isSetup ? (
          <div className="login-role-tabs" aria-label="Chọn vai trò đăng nhập">
            {(["admin", "user"] as AccountRole[]).map((item) => (
              <button
                className={role === item ? "active" : ""}
                disabled={isSubmitting}
                key={item}
                onClick={() => {
                  setRole(item);
                  setAuthError(null);
                  setPassword("");
                }}
                type="button"
              >
                {item === "admin" ? "Admin" : "User"}
              </button>
            ))}
          </div>
        ) : null}

        {needsPassword ? (
          <div className="login-fields">
            <label>
              <span>Password admin</span>
              <input
                autoFocus
                disabled={isSubmitting}
                onChange={(event) => setPassword(event.target.value)}
                type="password"
                value={password}
              />
            </label>
            {isSetup ? (
              <label>
                <span>Nhập lại password admin</span>
                <input
                  disabled={isSubmitting}
                  onChange={(event) => setConfirmPassword(event.target.value)}
                  type="password"
                  value={confirmPassword}
                />
              </label>
            ) : null}
          </div>
        ) : (
          <p className="login-user-note">User vào app không cần password.</p>
        )}

        {authError ? <strong className="login-error">{authError}</strong> : null}

        <button className="login-submit" disabled={!canSubmit || isSubmitting} type="submit">
          {isSubmitting ? "Đang xử lý..." : isSetup ? "Lưu password và vào Admin" : role === "admin" ? "Đăng nhập Admin" : "Vào bằng User"}
        </button>
      </form>
    </main>
  );
}

function LauncherApp({ accountRole, onLogout }: LauncherAppProps) {
  const [activePage, setActivePage] = useState<Page>("library");
  const [games, setGames] = useState<Game[]>([]);
  const [focusedGameId, setFocusedGameId] = useState<string | null>(null);
  const [launchedGameId, setLaunchedGameId] = useState<string | null>(null);
  const [pendingDeepLinkGameId, setPendingDeepLinkGameId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedCategory, setSelectedCategory] = useState("Tất cả");
  const [status, setStatus] = useState("Đang tải kho trò chơi...");
  const [error, setError] = useState<string | null>(null);
  const [busyGameId, setBusyGameId] = useState<string | null>(null);
  const [selectedLibraryGameIds, setSelectedLibraryGameIds] = useState<string[]>([]);
  const [isLibrarySelectionMode, setIsLibrarySelectionMode] = useState(false);
  const [isLeftMenuVisible, setIsLeftMenuVisible] = useState(true);
  const [isDeletingGames, setIsDeletingGames] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<PendingDelete | null>(null);
  const [configuredCategories, setConfiguredCategories] = useState<Category[]>([]);
  const [categoryName, setCategoryName] = useState("");
  const [categoryKeywords, setCategoryKeywords] = useState("");
  const [editingCategoryId, setEditingCategoryId] = useState<string | null>(null);
  const [pendingCategoryDelete, setPendingCategoryDelete] = useState<PendingCategoryDelete | null>(null);
  const [isSavingCategory, setIsSavingCategory] = useState(false);
  const [isDeletingCategory, setIsDeletingCategory] = useState(false);
  const [debugEntries, setDebugEntries] = useState<DebugEntry[]>([]);
  const [pendingSourceFiles, setPendingSourceFiles] = useState<File[]>([]);
  const [pendingSourceDirs, setPendingSourceDirs] = useState<string[]>([]);
  const [sourceFiles, setSourceFiles] = useState<InstallGameFile[]>([]);
  const [sourceFilePaths, setSourceFilePaths] = useState<InstallGamePath[]>([]);
  const [selectedSourceName, setSelectedSourceName] = useState<string | null>(null);
  const [scanResult, setScanResult] = useState<ScanGamesSummary | null>(null);
  const [selectedGameIds, setSelectedGameIds] = useState<string[]>([]);
  const [installSummary, setInstallSummary] = useState<InstallGamesSummary | null>(null);
  const [exportSummary, setExportSummary] = useState<ExportGamesArchiveSummary | null>(null);
  const [importSummary, setImportSummary] = useState<ImportGamesArchiveSummary | null>(null);
  const [appDiagnostics, setAppDiagnostics] = useState<AppDiagnostics | null>(null);
  const [exportProgress, setExportProgress] = useState<ExportGamesArchiveProgress | null>(null);
  const [classificationSummary, setClassificationSummary] = useState<ClassifyGamesSummary | null>(null);
  const [isScanningGames, setIsScanningGames] = useState(false);
  const [isUpdatingGames, setIsUpdatingGames] = useState(false);
  const [isExportingGames, setIsExportingGames] = useState(false);
  const [isImportingGames, setIsImportingGames] = useState(false);
  const [isLoadingDiagnostics, setIsLoadingDiagnostics] = useState(false);
  const [isClassifyingGames, setIsClassifyingGames] = useState(false);
  const [newAdminPassword, setNewAdminPassword] = useState("");
  const [exportAdminPassword, setExportAdminPassword] = useState("");
  const [isChangingPassword, setIsChangingPassword] = useState(false);
  const [gameListViewport, setGameListViewport] = useState({ height: 0, scrollTop: 0 });
  const folderInputRef = useRef<HTMLInputElement | null>(null);
  const gameListRef = useRef<HTMLDivElement | null>(null);
  const recentDeepLinkSelections = useRef(new Map<string, number>());
  const playHealthTimeoutRef = useRef<number | null>(null);
  const playRequestSerialRef = useRef(0);
  const isAdmin = accountRole === "admin";


  const appendDebug = useCallback((message: string, details?: unknown) => {
    const suffix = details === undefined ? "" : " " + JSON.stringify(debugValue(details));
    const entry = {
      at: new Date().toISOString(),
      message: message + suffix,
    };
    console.info("[YeuTre debug]", entry);
    setDebugEntries((current) => [...current, entry].slice(-40));
  }, []);

  const clearDebug = useCallback(() => {
    setDebugEntries([]);
  }, []);

  const copyDebug = useCallback(async () => {
    const content = debugEntries
      .map((entry) => `[${entry.at}] ${entry.message}`)
      .join("\n");

    try {
      await navigator.clipboard.writeText(content);
      setStatus("Đã sao chép log chẩn đoán.");
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] copy failed", err);
      setError(message);
      setStatus("Không thể sao chép log. Hãy mở Console để copy thủ công.");
    }
  }, [debugEntries]);

  const loadCategories = useCallback(async () => {
    try {
      if (isTauriRuntime) {
        setConfiguredCategories(await invoke<Category[]>("list_categories"));
        return;
      }

      const saved = window.localStorage.getItem("yeutre.categories");
      setConfiguredCategories(saved ? JSON.parse(saved) as Category[] : []);
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] load categories failed", err);
      setError(message);
      setStatus("Không thể đọc danh sách categories.");
    }
  }, []);

  const loadGames = useCallback(async () => {
    if (!isTauriRuntime) {
      setGames(previewGames);
      setStatus("Preview giao diện. Mở bằng Tauri để chơi game thật.");
      setError(null);
      return;
    }

    try {
      const items = await invoke<Game[]>("list_games");
      setGames(items);
      setStatus(`Sẵn sàng. Đã cài ${items.length} game.`);
      setError(null);
    } catch (err) {
      setError(String(err));
      setStatus("Không thể đọc danh sách game.");
    }
  }, []);

  const resetCategoryForm = useCallback(() => {
    setCategoryName("");
    setCategoryKeywords("");
    setEditingCategoryId(null);
  }, []);

  const editCategory = useCallback((category: Category) => {
    setEditingCategoryId(category.id);
    setCategoryName(category.name);
    setCategoryKeywords(category.keywords.join(", "));
    setError(null);
    setStatus("Đang sửa category: " + category.name);
  }, []);

  const saveCategory = useCallback(async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!isAdmin) {
      setError("Tài khoản User không được thay đổi cài đặt.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    const name = categoryName.trim();
    const keywords = categoryKeywords
      .split(",")
      .map((keyword) => keyword.trim())
      .filter(Boolean);

    if (!name || keywords.length === 0) {
      setError("Hãy nhập tên category và ít nhất một keyword.");
      setStatus("Chưa đủ thông tin category.");
      return;
    }

    try {
      setIsSavingCategory(true);
      setError(null);
      const savedCategory = isTauriRuntime
        ? await invoke<Category>("save_category", {
          input: {
            categoryId: editingCategoryId,
            name,
            keywords,
          },
        })
        : (() => {
          const id = editingCategoryId ?? gameIdFromFileName(name);
          if (!id) {
            throw new Error("Tên category không hợp lệ để tạo ID.");
          }
          const category = { id, name, keywords };
          const next = editingCategoryId
            ? configuredCategories.map((item) => item.id === editingCategoryId ? category : item)
            : [...configuredCategories, category];
          if (next.some((item, index) => next.findIndex((candidate) => candidate.name.toLowerCase() === item.name.toLowerCase()) !== index)) {
            throw new Error("Tên category đã tồn tại.");
          }
          window.localStorage.setItem("yeutre.categories", JSON.stringify(next));
          return category;
        })();

      if (isTauriRuntime) {
        await loadCategories();
      } else {
        setConfiguredCategories((current) => editingCategoryId
          ? current.map((item) => item.id === editingCategoryId ? savedCategory : item)
          : [...current, savedCategory]);
      }
      resetCategoryForm();
      setStatus(editingCategoryId ? "Đã cập nhật category." : "Đã tạo category mới.");
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] save category failed", err);
      setError(message);
      setStatus("Không thể lưu category.");
    } finally {
      setIsSavingCategory(false);
    }
  }, [configuredCategories, categoryKeywords, categoryName, editingCategoryId, isAdmin, loadCategories, resetCategoryForm]);

  const requestDeleteCategory = useCallback((category: Category) => {
    if (!isAdmin) {
      return;
    }

    setPendingCategoryDelete(category);
  }, [isAdmin]);

  const confirmDeleteCategory = useCallback(async () => {
    if (!isAdmin) {
      setPendingCategoryDelete(null);
      setError("Tài khoản User không được thay đổi cài đặt.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    if (!pendingCategoryDelete) {
      return;
    }

    const category = pendingCategoryDelete;
    setPendingCategoryDelete(null);
    try {
      setIsDeletingCategory(true);
      setError(null);
      if (isTauriRuntime) {
        await invoke("delete_category", { categoryId: category.id });
        await loadCategories();
      } else {
        const next = configuredCategories.filter((item) => item.id !== category.id);
        window.localStorage.setItem("yeutre.categories", JSON.stringify(next));
        setConfiguredCategories(next);
      }
      if (editingCategoryId === category.id) {
        resetCategoryForm();
      }
      setStatus("Đã xóa category: " + category.name);
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] delete category failed", err);
      setError(message);
      setStatus("Không thể xóa category.");
    } finally {
      setIsDeletingCategory(false);
    }
  }, [configuredCategories, editingCategoryId, isAdmin, loadCategories, pendingCategoryDelete, resetCategoryForm]);

  const classifyGamesByCategories = useCallback(async () => {
    if (!isAdmin) {
      setError("Tài khoản User không được phân loại game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    try {
      setIsClassifyingGames(true);
      setError(null);
      setClassificationSummary(null);

      const summary = isTauriRuntime
        ? await invoke<ClassifyGamesSummary>("classify_games_by_categories")
        : (() => {
          const result = classifyPreviewGames(games, configuredCategories);
          setGames(result.nextGames);
          return result.summary;
        })();

      setClassificationSummary(summary);
      setSelectedCategory("Tất cả");
      if (isTauriRuntime) {
        await loadGames();
      }
      setStatus(
        "Đã phân loại " + summary.updatedGames + " game. " +
          summary.matchedGames + " game khớp keywords, " + summary.skippedGames + " game chưa khớp.",
      );
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] classify games failed", err);
      setError(message);
      setStatus("Không thể phân loại game.");
    } finally {
      setIsClassifyingGames(false);
    }
  }, [configuredCategories, games, isAdmin, loadGames]);

  const changeAdminPassword = useCallback(async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!isAdmin) {
      setError("Tài khoản User không được thay đổi password admin.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    if (newAdminPassword.trim().length < 4) {
      setError("Password admin cần ít nhất 4 ký tự.");
      setStatus("Password mới chưa hợp lệ.");
      return;
    }

    try {
      setIsChangingPassword(true);
      setError(null);
      window.localStorage.setItem(ADMIN_PASSWORD_STORAGE_KEY, await hashAdminPassword(newAdminPassword));
      setNewAdminPassword("");
      setStatus("Đã đổi password admin. Lần đăng nhập Admin tiếp theo sẽ dùng password mới.");
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] change admin password failed", err);
      setError(message);
      setStatus("Không thể đổi password admin.");
    } finally {
      setIsChangingPassword(false);
    }
  }, [isAdmin, newAdminPassword]);

  const playGame = useCallback((gameId: string) => {
    const game = games.find((item) => item.id === gameId);
    if (!game) {
      setError("Không tìm thấy bài tập trong kho hiện tại: " + gameId);
      setStatus("Không thể mở bài tập trong khung xem.");
      return;
    }

    const iframeUrl = isTauriRuntime ? gameAssetUrl(game) : "preview-srcdoc";
    appendDebug("play requested", {
      gameId: game.id,
      title: game.title,
      entry: game.entry,
      iframeUrl,
      isTauriRuntime,
    });

    setBusyGameId(gameId);
    setLaunchedGameId(gameId);
    setFocusedGameId(gameId);
    setIsLeftMenuVisible(false);
    setActivePage("library");
    setError(null);
    const requestSerial = playRequestSerialRef.current + 1;
    playRequestSerialRef.current = requestSerial;
    if (playHealthTimeoutRef.current !== null) {
      window.clearTimeout(playHealthTimeoutRef.current);
    }
    window.setTimeout(() => setBusyGameId((current) => current === gameId ? null : current), 180);
    playHealthTimeoutRef.current = window.setTimeout(() => {
      if (playRequestSerialRef.current !== requestSerial) {
        return;
      }
      appendDebug("play health check", {
        gameId: game.id,
        iframeUrl,
        hint: "Nếu khung vẫn trắng, copy log chẩn đoán sau dòng này.",
      });
      playHealthTimeoutRef.current = null;
    }, 2500);
    setStatus("Đã mở bài tập trong khung xem: " + game.title);
  }, [appendDebug, games]);

  const copySelectedDeepLink = useCallback(async () => {
    if (!isAdmin) {
      setError("Tài khoản User không được copy deep link PowerPoint.");
      setStatus("Chức năng này chỉ dành cho Admin.");
      return;
    }

    const game = focusedGameId ? games.find((item) => item.id === focusedGameId) : null;
    if (!game) {
      setError("Chưa chọn game để copy deep link.");
      setStatus("Chưa có deep link để copy.");
      return;
    }

    const link = `yeutregame://play/${game.id}`;
    try {
      await navigator.clipboard.writeText(link);
      setError(null);
      setStatus("Đã copy deep link PowerPoint: " + link);
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] copy deep link failed", err);
      setError(message);
      setStatus("Không thể copy deep link PowerPoint.");
    }
  }, [focusedGameId, games, isAdmin]);

  const toggleLibraryGameSelection = useCallback((gameId: string) => {
    if (!isAdmin) {
      return;
    }

    setSelectedLibraryGameIds((current) =>
      current.includes(gameId) ? current.filter((id) => id !== gameId) : [...current, gameId],
    );
  }, [isAdmin]);

  const toggleLibrarySelectionMode = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    if (!isAdmin) {
      setIsLibrarySelectionMode(false);
      setSelectedLibraryGameIds([]);
      return;
    }

    const enabled = event.target.checked;
    setIsLibrarySelectionMode(enabled);
    if (!enabled) {
      setSelectedLibraryGameIds([]);
    }
  }, [isAdmin]);

  const selectLibraryGame = useCallback((gameId: string) => {
    const game = games.find((item) => item.id === gameId);
    setFocusedGameId(gameId);

    if (launchedGameId && launchedGameId !== gameId) {
      setLaunchedGameId(null);
      setStatus(
        game
          ? "Đã chọn " + game.title + ". Bấm Chơi ngay để mở bài tập này."
          : "Đã đổi bài tập đang chọn. Bấm Chơi ngay để mở trong khung xem.",
      );
    }

    if (!isAdmin || !isLibrarySelectionMode || isDeletingGames) {
      return;
    }

    if (game && game.isInstalled !== false) {
      toggleLibraryGameSelection(gameId);
    }
  }, [games, isAdmin, isDeletingGames, isLibrarySelectionMode, launchedGameId, toggleLibraryGameSelection]);

  const deleteSelectedGames = useCallback(() => {
    if (!isAdmin) {
      setError("Tài khoản User không được xóa game.");
      setStatus("Chức năng xóa file chỉ dành cho Admin.");
      return;
    }

    appendDebug("delete button clicked", {
      isTauriRuntime,
      selectedLibraryGameIds,
      games: games.map((game) => ({ id: game.id, isInstalled: game.isInstalled })),
    });

    const selectedInstalledIds = selectedLibraryGameIds.filter((gameId) =>
      games.some((game) => game.id === gameId && game.isInstalled !== false),
    );

    if (selectedInstalledIds.length === 0) {
      appendDebug("delete stopped: no deletable selected game", {
        selectedLibraryGameIds,
      });
      setError("Hãy chọn ít nhất một game đã cài để xóa.");
      setStatus("Chưa chọn game có thể xóa.");
      return;
    }

    const selectedTitles = games
      .filter((game) => selectedInstalledIds.includes(game.id))
      .map((game) => game.title)
      .join(", ");
    appendDebug("delete confirmation opened", {
      selectedInstalledIds,
      selectedTitles,
    });
    setPendingDelete({ gameIds: selectedInstalledIds, titles: selectedTitles });
  }, [appendDebug, games, isAdmin, selectedLibraryGameIds]);

  const cancelDeleteSelectedGames = useCallback(() => {
    if (pendingDelete) {
      appendDebug("delete confirmation cancelled", pendingDelete);
    }
    setPendingDelete(null);
  }, [appendDebug, pendingDelete]);

  const confirmDeleteSelectedGames = useCallback(async () => {
    if (!isAdmin) {
      setPendingDelete(null);
      setError("Tài khoản User không được xóa game.");
      setStatus("Chức năng xóa file chỉ dành cho Admin.");
      return;
    }

    if (!pendingDelete) {
      appendDebug("delete confirmation ignored: no pending deletion");
      return;
    }

    const selectedInstalledIds = pendingDelete.gameIds;
    setPendingDelete(null);
    appendDebug("delete confirmation accepted", { selectedInstalledIds });

    try {
      appendDebug("delete invoke started", {
        command: "delete_games",
        gameIds: selectedInstalledIds,
        isTauriRuntime,
      });
      setIsDeletingGames(true);
      setError(null);
      setStatus("Đang xóa các game đã chọn...");

      const summary = isTauriRuntime
        ? await invoke<DeleteGamesSummary>("delete_games", { gameIds: selectedInstalledIds })
        : {
          deletedGames: selectedInstalledIds.length,
          skippedGames: 0,
          gameIds: selectedInstalledIds,
        };

      appendDebug("delete invoke succeeded", summary);

      if (isTauriRuntime) {
        await loadGames();
      } else {
        setGames((current) => current.filter((game) => !selectedInstalledIds.includes(game.id)));
      }

      setSelectedLibraryGameIds([]);
      setLaunchedGameId((current) => current && selectedInstalledIds.includes(current) ? null : current);
      setStatus(
        "Đã xóa " + summary.deletedGames + " game khỏi app." +
          (summary.skippedGames > 0 ? " " + summary.skippedGames + " game được bỏ qua." : ""),
      );
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] delete invoke failed", err);
      appendDebug("delete invoke failed", { error: debugValue(err), message });
      setError(message);
      setStatus("Không thể xóa các game đã chọn.");
    } finally {
      setIsDeletingGames(false);
    }
  }, [appendDebug, isAdmin, loadGames, pendingDelete]);

  useEffect(() => {
    const onError = (event: ErrorEvent) => {
      appendDebug("window error", {
        message: event.message,
        filename: event.filename,
        line: event.lineno,
        column: event.colno,
      });
    };
    const onUnhandledRejection = (event: PromiseRejectionEvent) => {
      appendDebug("unhandled promise rejection", debugValue(event.reason));
    };

    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onUnhandledRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onUnhandledRejection);
    };
  }, [appendDebug]);

  useEffect(() => {
    return () => {
      if (playHealthTimeoutRef.current !== null) {
        window.clearTimeout(playHealthTimeoutRef.current);
      }
    };
  }, []);

  const chooseSourceFolder = useCallback(async () => {
    if (!isAdmin) {
      setError("Tài khoản User không được cập nhật kho game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    if (isTauriRuntime) {
      try {
        const selected = await open({ directory: true, multiple: true });
        const sourceDirs = Array.isArray(selected) ? selected : selected ? [selected] : [];
        setInstallSummary(null);
        setScanResult(null);
        setSelectedGameIds([]);
        setSourceFiles([]);
        setSourceFilePaths([]);
        setPendingSourceFiles([]);
        setPendingSourceDirs(sourceDirs);
        setError(null);

        if (sourceDirs.length === 0) {
          setSelectedSourceName(null);
          setStatus("Chưa chọn folder chứa game.");
          return;
        }

        const sourceName = sourceDirs.length === 1
          ? displayNameForSourcePath(sourceDirs[0])
          : sourceDirs.length + " folder đã chọn";
        setSelectedSourceName(sourceName);
        setStatus("Đã chọn " + sourceName + ". Bấm Quét game để tìm folder game hoặc file HTML mới.");
      } catch (err) {
        const message = errorMessage(err);
        console.error("[YeuTre debug] open source folder failed", err);
        setError(message);
        setStatus("Không thể mở hộp thoại chọn folder.");
      }
      return;
    }

    const input = folderInputRef.current;
    if (!input) {
      return;
    }

    input.value = "";
    input.removeAttribute("accept");
    input.setAttribute("webkitdirectory", "");
    input.setAttribute("directory", "");
    input.click();
  }, [isAdmin]);


  const chooseSourceFiles = useCallback(async () => {
    if (!isAdmin) {
      setError("Tài khoản User không được cập nhật kho game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    if (isTauriRuntime) {
      try {
        const selected = await open({
          filters: [{ name: "HTML games", extensions: ["html", "htm"] }],
          multiple: true,
        });
        const selectedPaths = Array.isArray(selected) ? selected : selected ? [selected] : [];
        const pathFiles = selectedPaths
          .filter((path) => isHtmlFileName(path))
          .map((path) => ({ relativePath: displayNameForSourcePath(path), path }));

        setInstallSummary(null);
        setScanResult(null);
        setSelectedGameIds([]);
        setSourceFiles([]);
        setSourceFilePaths(pathFiles);
        setPendingSourceFiles([]);
        setPendingSourceDirs([]);
        setError(null);

        if (pathFiles.length === 0) {
          setSelectedSourceName(null);
          setStatus("Chưa chọn file HTML nào.");
          return;
        }

        const sourceName = pathFiles.length === 1
          ? pathFiles[0].relativePath
          : pathFiles.length + " file HTML đã chọn";
        setSelectedSourceName(sourceName);
        setStatus("Đã chọn " + sourceName + ". Bấm Quét game để kiểm tra file mới.");
      } catch (err) {
        const message = errorMessage(err);
        console.error("[YeuTre debug] open source files failed", err);
        setError(message);
        setStatus("Không thể mở hộp thoại chọn file HTML.");
      }
      return;
    }

    const input = folderInputRef.current;
    if (!input) {
      return;
    }

    input.value = "";
    input.removeAttribute("webkitdirectory");
    input.removeAttribute("directory");
    input.setAttribute("accept", ".html,.htm,text/html");
    input.click();
  }, [isAdmin]);

  const handleFolderSelected = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    if (!isAdmin) {
      event.target.value = "";
      setError("Tài khoản User không được cập nhật kho game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    const files = Array.from(event.target.files ?? []);
    setInstallSummary(null);
    setScanResult(null);
    setSelectedGameIds([]);
    setSourceFiles([]);
    setSourceFilePaths([]);
    setPendingSourceFiles(files);
    setPendingSourceDirs([]);
    setError(null);

    if (files.length === 0) {
      setSelectedSourceName(null);
      setStatus("Chưa chọn folder chứa file HTML.");
      return;
    }

    const firstRelativePath = fileRelativePath(files[0]);
    const htmlFiles = files.filter((file) => isHtmlFileName(file.name));
    const isDirectorySelection = firstRelativePath.includes("/");
    const rootName = isDirectorySelection
      ? firstRelativePath.split("/")[0] || "Thư mục đã chọn"
      : htmlFiles.length === 1
        ? htmlFiles[0].name
        : htmlFiles.length + " file HTML đã chọn";
    const oversizedHtmlCount = htmlFiles.filter((file) => file.size > MAX_BROWSER_HTML_FILE_BYTES).length;
    const pathModeReady = isTauriRuntime && htmlFiles.length > 0 && htmlSourcePathFiles(files).length === htmlFiles.length;
    setSelectedSourceName(rootName);
    setStatus(
      "Đã chọn " + rootName + " với " + htmlFiles.length +
        " file HTML" +
        (pathModeReady ? " (chế độ nhẹ)." : ".") +
        (oversizedHtmlCount > 0 ? " " + oversizedHtmlCount + " file quá lớn sẽ bị bỏ qua." : "") +
        " Bấm Quét game để tìm game mới.",
    );
  }, [isAdmin]);

  const scanGamesFromSelectedFolder = useCallback(async () => {
    if (!isAdmin) {
      setError("Tài khoản User không được cập nhật kho game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    if (pendingSourceDirs.length === 0 && pendingSourceFiles.length === 0 && sourceFilePaths.length === 0) {
      setError("Bạn cần chọn folder, USB hoặc file HTML trước.");
      setStatus("Chưa có nguồn để quét.");
      return;
    }

    try {
      setIsScanningGames(true);
      setInstallSummary(null);
      setScanResult(null);
      setSelectedGameIds([]);
      setError(null);
      setStatus("Đang phân tích folder game và file HTML trong nguồn đã chọn...");

      const selectedPathFiles = sourceFilePaths.length > 0 ? sourceFilePaths : htmlSourcePathFiles(pendingSourceFiles);
      const pathFiles = isTauriRuntime ? selectedPathFiles : [];
      const canUseSourceDirMode = isTauriRuntime && pendingSourceDirs.length > 0;
      const canUsePathMode = isTauriRuntime && pathFiles.length > 0;
      let incomingFiles: InstallGameFile[] = [];

      if (!canUseSourceDirMode && !canUsePathMode && pendingSourceFiles.length > MAX_BROWSER_SCAN_FILES) {
        throw new Error("Folder có quá nhiều file để quét bằng chế độ browser. Hãy mở bằng app Tauri để dùng chế độ nhẹ.");
      }

      const result = canUseSourceDirMode
        ? await invoke<ScanGamesSummary>("scan_game_sources", { sourceDirs: pendingSourceDirs })
        : canUsePathMode
          ? await invoke<ScanGamesSummary>("scan_games_from_paths", { files: pathFiles })
          : isTauriRuntime
            ? await invoke<ScanGamesSummary>("scan_games_from_files", {
              files: incomingFiles = await Promise.all(
                pendingSourceFiles.map(async (file) => {
                  if (file.size > MAX_BROWSER_HTML_FILE_BYTES) {
                    return { relativePath: fileRelativePath(file), bytes: [] };
                  }
                  return {
                    relativePath: fileRelativePath(file),
                    bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
                  };
                }),
              ),
            })
            : buildPreviewScanResult(pendingSourceFiles, games);
      const newGameIds = result.games.filter((game) => game.isNew).map((game) => game.id);

      setSourceFilePaths(canUsePathMode ? pathFiles : []);
      setSourceFiles(canUsePathMode ? [] : incomingFiles);
      appendDebug("scan completed", {
        mode: canUseSourceDirMode ? "source-dirs" : canUsePathMode ? "paths" : "bytes",
        htmlFiles: result.games.length,
        newGames: newGameIds.length,
        existingGames: result.games.length - newGameIds.length,
        skippedFiles: result.skippedFiles,
      });
      setScanResult(result);
      setSelectedGameIds(newGameIds);
      if (newGameIds.length === 0) {
        setStatus(
          "Đã quét " + result.games.length +
            " game nhưng không có game mới. Game trùng ID sẽ không được ghi đè; hãy đổi tên file/folder hoặc xóa game cũ trước khi cập nhật lại.",
        );
      } else {
        setStatus(
          "Đã quét " + result.games.length + " game, phát hiện " + newGameIds.length +
            " game mới. Bấm Xác nhận cập nhật để đồng bộ game mới.",
        );
      }
    } catch (err) {
      setError(String(err));
      setStatus("Không thể quét nguồn game đã chọn.");
    } finally {
      setIsScanningGames(false);
    }
  }, [appendDebug, games, pendingSourceDirs, pendingSourceFiles, sourceFilePaths]);

  const toggleGameSelection = useCallback((gameId: string) => {
    setSelectedGameIds((current) =>
      current.includes(gameId) ? current.filter((id) => id !== gameId) : [...current, gameId],
    );
  }, []);

  const updateGamesFromSelectedFolder = useCallback(async () => {
    if (!isAdmin) {
      setError("Tài khoản User không được cập nhật kho game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    if ((pendingSourceDirs.length === 0 && sourceFiles.length === 0 && sourceFilePaths.length === 0) || !scanResult) {
      setError("Bạn cần bấm Quét game trước khi xác nhận cập nhật.");
      setStatus("Chưa có kết quả quét để cập nhật.");
      return;
    }

    const newSelectedGameIds = selectedGameIds.filter((gameId) =>
      scanResult.games.some((game) => game.id === gameId && game.isNew),
    );

    if (newSelectedGameIds.length === 0) {
      setError("Không có game mới nào để cập nhật.");
      setStatus("Tất cả game đã có trong kho hoặc chưa được chọn.");
      return;
    }

    const updateMode = pendingSourceDirs.length > 0
      ? "source-dirs"
      : sourceFilePaths.length > 0
        ? "paths"
        : "bytes";

    try {
      setIsUpdatingGames(true);
      setError(null);
      setStatus("Đang đồng bộ các game mới đã xác nhận...");
      appendDebug("install requested", {
        mode: isTauriRuntime ? updateMode : "preview",
        selectedGames: newSelectedGameIds.length,
        sourceDirs: pendingSourceDirs.length,
        sourcePathFiles: sourceFilePaths.length,
        sourceByteFiles: sourceFiles.length,
      });
      await waitForUiFrame();

      const summary = isTauriRuntime
        ? pendingSourceDirs.length > 0
          ? await invoke<InstallGamesSummary>("install_game_sources", {
            sourceDirs: pendingSourceDirs,
            gameIds: newSelectedGameIds,
          })
          : sourceFilePaths.length > 0
            ? await invoke<InstallGamesSummary>("install_games_from_paths", {
              files: sourceFilePaths,
              gameIds: newSelectedGameIds,
            })
            : await invoke<InstallGamesSummary>("install_games_from_files", {
              files: sourceFiles,
              gameIds: newSelectedGameIds,
            })
        : {
          copiedFiles: newSelectedGameIds.length,
          installedGames: newSelectedGameIds.length,
          skippedFiles: scanResult.skippedFiles,
          targetDir: "Preview browser",
          gameIds: newSelectedGameIds,
        };
      appendDebug("install completed", summary);

      if (!isTauriRuntime) {
        const previewNewGames = scanResult.games
          .filter((game) => newSelectedGameIds.includes(game.id))
          .map(({ fileCount, isNew, sourcePath, ...game }) => game);
        setGames((current) => [...current, ...previewNewGames]);
      }

      setInstallSummary(summary);
      setPendingSourceFiles([]);
      setPendingSourceDirs([]);
      setSourceFiles([]);
      setSourceFilePaths([]);
      setScanResult(null);
      setSelectedGameIds([]);
      setSelectedSourceName(null);
      if (isTauriRuntime) {
        await loadGames();
      }
      setActivePage("library");
      setStatus(
        "Đã đồng bộ " + summary.installedGames +
          " game mới. Kho game đã được làm mới.",
      );
    } catch (err) {
      const message = errorMessage(err);
      appendDebug("install failed", { mode: updateMode, message });
      setError(message);
      setStatus("Không thể đồng bộ file HTML đã xác nhận.");
    } finally {
      setIsUpdatingGames(false);
    }
  }, [appendDebug, isAdmin, loadGames, pendingSourceDirs, scanResult, selectedGameIds, sourceFilePaths, sourceFiles]);

  const loadDiagnostics = useCallback(async () => {
    if (!isAdmin) {
      return;
    }

    if (!isTauriRuntime) {
      setAppDiagnostics(null);
      return;
    }

    try {
      setIsLoadingDiagnostics(true);
      setError(null);
      setAppDiagnostics(await invoke<AppDiagnostics>("get_app_diagnostics"));
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] diagnostics failed", err);
      setError(message);
      setStatus("Không thể đọc diagnostics của app.");
    } finally {
      setIsLoadingDiagnostics(false);
    }
  }, [isAdmin]);

  const copyDiagnostics = useCallback(async () => {
    if (!appDiagnostics) {
      setStatus("Chưa có diagnostics để sao chép.");
      return;
    }

    try {
      await navigator.clipboard.writeText(JSON.stringify(appDiagnostics, null, 2));
      setStatus("Đã sao chép diagnostics.");
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] copy diagnostics failed", err);
      setError(message);
      setStatus("Không thể sao chép diagnostics.");
    }
  }, [appDiagnostics]);

  const importGamesArchive = useCallback(async () => {
    if (!isAdmin) {
      setError("Tài khoản User không được nhập kho game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    if (!isTauriRuntime) {
      setError("Tính năng nhập file zip chỉ chạy trong app Tauri.");
      setStatus("Mở app bằng npm run tauri dev để nhập kho game.");
      return;
    }

    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Kho game YeuTre", extensions: ["zip"] }],
      });
      const archivePath = Array.isArray(selected) ? selected[0] : selected;
      if (!archivePath) {
        setStatus("Chưa chọn file zip để nhập.");
        return;
      }

      setIsImportingGames(true);
      setError(null);
      setImportSummary(null);
      setStatus("Đang nhập kho game từ file zip...");

      const summary = await invoke<ImportGamesArchiveSummary>("import_games_archive", { archivePath });
      setImportSummary(summary);
      await loadGames();
      await loadCategories();
      await loadDiagnostics();
      setActivePage("library");
      setStatus(
        "Đã nhập " + summary.importedGames +
          " game từ zip. Bỏ qua " + summary.skippedGames + " game trùng/không hợp lệ.",
      );
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] import games archive failed", err);
      setError(message);
      setStatus("Không thể nhập kho game từ file zip.");
    } finally {
      setIsImportingGames(false);
    }
  }, [isAdmin, loadCategories, loadDiagnostics, loadGames]);

  const exportGamesArchive = useCallback(async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!isAdmin) {
      setError("Tài khoản User không được xuất kho game.");
      setStatus("Cài đặt chỉ dành cho Admin.");
      return;
    }

    const installedCount = games.filter((game) => game.isInstalled !== false).length;
    if (installedCount === 0) {
      setError("Chưa có game nào trong kho để xuất.");
      setStatus("Kho game đang trống.");
      return;
    }

    if (!isTauriRuntime) {
      setError("Tính năng xuất file zip chỉ chạy trong app Tauri.");
      setStatus("Mở app bằng npm run tauri dev để xuất kho game.");
      return;
    }

    const savedHash = window.localStorage.getItem(ADMIN_PASSWORD_STORAGE_KEY);
    if (!savedHash) {
      setError("Chưa thiết lập password admin.");
      setStatus("Không thể xác thực quyền xuất kho game.");
      return;
    }

    if (!exportAdminPassword) {
      setError("Hãy nhập password admin để xác nhận xuất kho game.");
      setStatus("Cần xác nhận password admin trước khi xuất zip.");
      return;
    }

    if (await hashAdminPassword(exportAdminPassword) !== savedHash) {
      setError("Password admin không đúng.");
      setStatus("Không thể xuất kho game vì xác thực thất bại.");
      return;
    }

    try {
      setIsExportingGames(true);
      setError(null);
      setExportSummary(null);
      setExportProgress(null);
      setStatus("Đang nén toàn bộ kho game thành file zip...");

      const summary = await invoke<ExportGamesArchiveSummary>("export_games_archive");
      setExportSummary(summary);
      setExportProgress(null);
      setExportAdminPassword("");
      setStatus(
        "Đã xuất " + summary.exportedGames +
          " game thành file zip: " + summary.archivePath,
      );
    } catch (err) {
      const message = errorMessage(err);
      console.error("[YeuTre debug] export games archive failed", err);
      setError(message);
      setStatus("Không thể xuất kho game thành file zip.");
    } finally {
      setIsExportingGames(false);
    }
  }, [exportAdminPassword, games, isAdmin]);

  useEffect(() => {
    void loadGames();
    if (isAdmin) {
      void loadCategories();
    }
  }, [isAdmin, loadCategories, loadGames]);

  useEffect(() => {
    if (isAdmin) {
      return;
    }

    if (activePage === "settings") {
      setActivePage("library");
    }
    setIsLibrarySelectionMode(false);
    setSelectedLibraryGameIds([]);
    setPendingDelete(null);
    setPendingCategoryDelete(null);
  }, [activePage, isAdmin]);

  useEffect(() => {
    if (isAdmin && activePage === "settings") {
      void loadDiagnostics();
    }
  }, [activePage, isAdmin, loadDiagnostics]);

  useEffect(() => {
    if (pendingDeepLinkGameId) {
      return;
    }

    if (games.length === 0) {
      setFocusedGameId(null);
      return;
    }

    if (!focusedGameId || !games.some((game) => game.id === focusedGameId)) {
      setFocusedGameId(games[0].id);
    }
  }, [focusedGameId, games, pendingDeepLinkGameId]);

  useEffect(() => {
    if (!pendingDeepLinkGameId || games.length === 0) {
      return;
    }

    const linkedGame = games.find((game) => game.id === pendingDeepLinkGameId);
    if (!linkedGame) {
      setPendingDeepLinkGameId(null);
      setError("Link PowerPoint trỏ tới game không có trong kho hiện tại: " + pendingDeepLinkGameId);
      setStatus("Không tìm thấy bài tập từ PowerPoint trong kho game hiện tại.");
      return;
    }

    setFocusedGameId(linkedGame.id);
    setSelectedCategory("Tất cả");
    setSearchQuery("");
    setActivePage("library");
    setPendingDeepLinkGameId(null);
    setError(null);
    setStatus("Đã chọn bài tập từ PowerPoint: " + linkedGame.title + ". Đang mở bài tập...");
    void playGame(linkedGame.id);
  }, [games, pendingDeepLinkGameId, playGame]);

  useEffect(() => {
    const handleGameRuntimeMessage = (event: MessageEvent) => {
      const data = event.data as Partial<GameRuntimeMessage> | null;
      if (!data || data.source !== "yeutre-game-runtime") {
        return;
      }

      appendDebug("game runtime " + data.level, data);
      if (data.level === "ready") {
        setStatus("Game đã báo sẵn sàng trong khung xem.");
        return;
      }

      if (data.level === "storage-fallback") {
        setStatus("Game đang dùng bộ nhớ tạm trong app vì localStorage bị giới hạn.");
        return;
      }

      if (data.level === "drag-drop") {
        setStatus("Game đã nhận thao tác kéo thả trong khung xem.");
        return;
      }

      setError(data.message || "Game báo lỗi khi chạy trong khung xem.");
      setStatus("Game gặp lỗi khi chạy. Hãy copy log chẩn đoán gửi lại.");
    };

    window.addEventListener("message", handleGameRuntimeMessage);
    return () => window.removeEventListener("message", handleGameRuntimeMessage);
  }, [appendDebug]);

  useEffect(() => {
    if (!isTauriRuntime || !isAdmin) {
      return;
    }

    let cleanupExportProgress: (() => void) | undefined;
    listen<ExportGamesArchiveProgress>("game-export-progress", (event) => {
      setExportProgress(event.payload);
      if (isExportingGames) {
        setStatus(
          "Đang xuất kho game: " + event.payload.exportedFiles +
            " file" + (event.payload.currentPath ? " · " + event.payload.currentPath : ""),
        );
      }
    }).then((unlisten) => {
      cleanupExportProgress = unlisten;
    });

    return () => {
      cleanupExportProgress?.();
    };
  }, [isAdmin, isExportingGames]);

  useEffect(() => {
    if (!isTauriRuntime) {
      return;
    }

    let cleanupDeepLinkStatus: (() => void) | undefined;

    const applyDeepLinkStatus = (payload: DeepLinkStatus) => {
      appendDebug("deep-link-status", payload);

      if (payload.status === "received") {
        setError(null);
        setStatus("Đã nhận link PowerPoint: " + payload.url);
        return;
      }

      if (payload.status === "selected") {
        const selectionKey = payload.url + "::" + (payload.gameId ?? "");
        const now = Date.now();
        const lastSelectedAt = recentDeepLinkSelections.current.get(selectionKey);
        if (lastSelectedAt && now - lastSelectedAt < 1500) {
          return;
        }
        recentDeepLinkSelections.current.set(selectionKey, now);

        setError(null);
        setActivePage("library");
        setSelectedCategory("Tất cả");
        setSearchQuery("");
        if (payload.gameId) {
          setPendingDeepLinkGameId(payload.gameId);
          setFocusedGameId(payload.gameId);
        }
        setStatus(payload.message || "Đã chọn bài tập từ PowerPoint. Đang mở bài tập...");
        return;
      }

      setError(payload.message);
      setStatus("Không thể mở game từ link PowerPoint.");
    };

    listen<DeepLinkStatus>("deep-link-status", (event) => {
      applyDeepLinkStatus(event.payload);
    }).then((unlisten) => {
      cleanupDeepLinkStatus = unlisten;
    });

    void invoke<DeepLinkStatus | null>("last_deep_link_status")
      .then((payload) => {
        if (payload) {
          applyDeepLinkStatus(payload);
        }
      })
      .catch((err) => {
        console.error("[YeuTre debug] last deep link status failed", err);
      });

    return () => {
      cleanupDeepLinkStatus?.();
    };
  }, [appendDebug]);

  const categories = useMemo(
    () => ["Tất cả", ...Array.from(new Set(games.map((game) => game.category))).sort()],
    [games],
  );
  const filteredGames = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();

    return games.filter((game) => {
      const matchesCategory = selectedCategory === "Tất cả" || game.category === selectedCategory;
      const haystack = [game.title, game.id, game.grade, game.category].join(" ").toLowerCase();
      return matchesCategory && (!query || haystack.includes(query));
    });
  }, [games, searchQuery, selectedCategory]);
  const deletableFilteredGames = useMemo(
    () => filteredGames.filter((game) => game.isInstalled !== false),
    [filteredGames],
  );
  const selectedDeletableGameCount = selectedLibraryGameIds.filter((gameId) =>
    games.some((game) => game.id === gameId && game.isInstalled !== false),
  ).length;
  const allVisibleGamesSelected =
    deletableFilteredGames.length > 0 &&
    deletableFilteredGames.every((game) => selectedLibraryGameIds.includes(game.id));
  const shouldVirtualizeGameList = filteredGames.length >= VIRTUAL_GAME_LIST_THRESHOLD;
  const virtualStartIndex = shouldVirtualizeGameList
    ? Math.max(0, Math.floor(gameListViewport.scrollTop / VIRTUAL_GAME_ROW_HEIGHT) - VIRTUAL_GAME_LIST_OVERSCAN)
    : 0;
  const virtualVisibleCount = shouldVirtualizeGameList
    ? Math.ceil((gameListViewport.height || 620) / VIRTUAL_GAME_ROW_HEIGHT) + VIRTUAL_GAME_LIST_OVERSCAN * 2
    : filteredGames.length;
  const virtualEndIndex = shouldVirtualizeGameList
    ? Math.min(filteredGames.length, virtualStartIndex + virtualVisibleCount)
    : filteredGames.length;
  const visibleGameRows = filteredGames.slice(virtualStartIndex, virtualEndIndex);
  const virtualTopSpacer = shouldVirtualizeGameList ? virtualStartIndex * VIRTUAL_GAME_ROW_HEIGHT : 0;
  const virtualBottomSpacer = shouldVirtualizeGameList
    ? Math.max(0, (filteredGames.length - virtualEndIndex) * VIRTUAL_GAME_ROW_HEIGHT)
    : 0;

  useEffect(() => {
    gameListRef.current?.scrollTo({ top: 0 });
    setGameListViewport((current) => current.scrollTop === 0 ? current : { ...current, scrollTop: 0 });
  }, [searchQuery, selectedCategory]);

  useEffect(() => {
    if (activePage !== "library") {
      return;
    }

    const list = gameListRef.current;
    if (!list) {
      return;
    }

    const syncGameListViewport = () => {
      setGameListViewport((current) => {
        const nextHeight = list.clientHeight;
        const nextScrollTop = list.scrollTop;
        if (current.height === nextHeight && current.scrollTop === nextScrollTop) {
          return current;
        }
        return { height: nextHeight, scrollTop: nextScrollTop };
      });
    };

    syncGameListViewport();
    window.requestAnimationFrame(syncGameListViewport);

    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", syncGameListViewport);
      return () => {
        window.removeEventListener("resize", syncGameListViewport);
      };
    }

    const observer = new ResizeObserver(syncGameListViewport);
    observer.observe(list);
    return () => {
      observer.disconnect();
    };
  }, [activePage, filteredGames.length, isLeftMenuVisible, isLibrarySelectionMode, isAdmin]);

  useEffect(() => {
    if (activePage !== "library" || !focusedGameId) {
      return;
    }

    const focusedIndex = filteredGames.findIndex((game) => game.id === focusedGameId);
    if (filteredGames.length >= VIRTUAL_GAME_LIST_THRESHOLD && focusedIndex >= 0) {
      window.requestAnimationFrame(() => {
        gameListRef.current?.scrollTo({
          top: Math.max(0, focusedIndex * VIRTUAL_GAME_ROW_HEIGHT - VIRTUAL_GAME_ROW_HEIGHT),
          behavior: "smooth",
        });
      });
      return;
    }

    window.requestAnimationFrame(() => {
      document
        .querySelector(`[data-game-id="${focusedGameId}"]`)
        ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    });
  }, [activePage, filteredGames, focusedGameId]);

  const toggleAllVisibleGames = useCallback(() => {
    if (!isAdmin || !isLibrarySelectionMode) {
      return;
    }

    const visibleIds = deletableFilteredGames.map((game) => game.id);
    setSelectedLibraryGameIds((current) => {
      if (visibleIds.every((gameId) => current.includes(gameId))) {
        return current.filter((gameId) => !visibleIds.includes(gameId));
      }

      return Array.from(new Set([...current, ...visibleIds]));
    });
  }, [deletableFilteredGames, isAdmin, isLibrarySelectionMode]);

  const focusedGame = useMemo(
    () => games.find((game) => game.id === focusedGameId) ?? filteredGames[0] ?? games[0] ?? null,
    [filteredGames, focusedGameId, games],
  );
  const launchedGame = useMemo(
    () => games.find((game) => game.id === launchedGameId) ?? null,
    [games, launchedGameId],
  );
  const gradeCount = useMemo(() => new Set(games.map((game) => game.grade)).size, [games]);
  const installedGameCount = useMemo(
    () => games.filter((game) => game.isInstalled !== false).length,
    [games],
  );
  const scanNewCount = scanResult?.games.filter((game) => game.isNew).length ?? 0;
  const canUpdateScannedGames = Boolean(scanResult) && selectedGameIds.length > 0 && !isScanningGames && !isUpdatingGames;
  const updateGamesButtonLabel = isUpdatingGames
    ? "Đang cập nhật..."
    : scanResult && scanNewCount === 0
      ? "Không có game mới"
      : "Xác nhận cập nhật";
  const updateGamesButtonTitle = scanResult && scanNewCount === 0
    ? "Game đã có trong kho nên app không ghi đè. Hãy đổi tên file/folder hoặc xóa game cũ trước."
    : undefined;
  const selectedDeepLink = focusedGame ? `yeutregame://play/${focusedGame.id}` : "Chọn game để xem deep link";
  const playerSrc = launchedGame && isTauriRuntime ? gameAssetUrl(launchedGame) : null;
  const previewPlayerHtml = launchedGame && !isTauriRuntime ? buildPreviewPlayerHtml(launchedGame) : undefined;
  const appShellClassName = isLeftMenuVisible ? "app-shell" : "app-shell left-menu-hidden";

  return (
    <main className={appShellClassName}>
      <nav className="topbar" aria-label="Điều hướng chính">
        <div className="brand-mark">
          <span>{APP_INITIALS}</span>
          <div className="brand-title-line">
            <strong>{APP_NAME}</strong>
            <span className="version-badge">v{APP_VERSION}</span>
          </div>
        </div>
        <label className="menu-visibility-switch">
          <span>Menu trái</span>
          <input
            checked={isLeftMenuVisible}
            onChange={(event) => setIsLeftMenuVisible(event.target.checked)}
            type="checkbox"
          />
          <span className="switch-track" aria-hidden="true" />
        </label>
        <div className="topbar-tabs">
          <button
            className={activePage === "library" ? "topbar-tab active" : "topbar-tab"}
            onClick={() => setActivePage("library")}
            type="button"
          >
            Kho game
          </button>
          {isAdmin ? (
            <button
              className={activePage === "settings" ? "topbar-tab active" : "topbar-tab"}
              onClick={() => setActivePage("settings")}
              type="button"
            >
              Cài đặt
            </button>
          ) : null}
        </div>
        <div className="topbar-session">
          <span className={error ? "topbar-status error" : "topbar-status"}>{error ? "Cần kiểm tra" : "Sẵn sàng"}</span>
          <span className="role-badge">{isAdmin ? "Admin" : "User"}</span>
          <button className="logout-button" onClick={onLogout} type="button">Thoát</button>
        </div>
      </nav>

      <section className="status-row" aria-live="polite">
        <span>{status}</span>
        {error ? <strong>{error}</strong> : null}
      </section>

      {isAdmin && debugEntries.length > 0 ? (
        <details className="debug-panel" open={Boolean(error)}>
          <summary>Chẩn đoán thao tác ({debugEntries.length} dòng)</summary>
          <pre>{debugEntries.map((entry) => `[${entry.at}] ${entry.message}`).join("\n")}</pre>
          <div className="debug-actions">
            <button onClick={() => void copyDebug()} type="button">Sao chép log</button>
            <button onClick={clearDebug} type="button">Xóa log</button>
          </div>
        </details>
      ) : null}

      {isAdmin && pendingDelete ? (
        <div className="modal-backdrop">
          <section
            aria-labelledby="delete-confirmation-title"
            aria-modal="true"
            className="confirm-dialog"
            role="dialog"
          >
            <p className="eyebrow">Xác nhận thao tác</p>
            <h2 id="delete-confirmation-title">Xóa game đã chọn?</h2>
            <p className="confirm-dialog-copy">
              Bạn sắp xóa {pendingDelete.gameIds.length} game khỏi app. Toàn bộ file game trong app data sẽ bị xóa.
            </p>
            <code>{pendingDelete.titles}</code>
            <div className="confirm-dialog-actions">
              <button autoFocus onClick={cancelDeleteSelectedGames} type="button">Hủy</button>
              <button className="delete-button" onClick={() => void confirmDeleteSelectedGames()} type="button">
                Xóa game
              </button>
            </div>
          </section>
        </div>
      ) : null}

      {isAdmin && pendingCategoryDelete ? (
        <div className="modal-backdrop">
          <section
            aria-labelledby="category-delete-title"
            aria-modal="true"
            className="confirm-dialog"
            role="dialog"
          >
            <p className="eyebrow">Xác nhận thao tác</p>
            <h2 id="category-delete-title">Xóa category?</h2>
            <p className="confirm-dialog-copy">
              Category này sẽ bị xóa khỏi danh sách cấu hình. Các file game hiện tại không bị xóa.
            </p>
            <code>{pendingCategoryDelete.name}</code>
            <div className="confirm-dialog-actions">
              <button autoFocus onClick={() => setPendingCategoryDelete(null)} type="button">Hủy</button>
              <button
                className="delete-button"
                disabled={isDeletingCategory}
                onClick={() => void confirmDeleteCategory()}
                type="button"
              >
                {isDeletingCategory ? "Đang xóa..." : "Xóa category"}
              </button>
            </div>
          </section>
        </div>
      ) : null}

      {activePage === "library" || !isAdmin ? (
        <section className="launcher-console">
          <div className="catalog-panel">
            <div className="console-toolbar">
              <div>
                <p className="eyebrow">Kho game giáo dục</p>
                <h1>Chọn game để chơi</h1>
              </div>
              <div className="quick-stats" aria-label="Thống kê kho game">
                <span>
                  <strong>{games.length}</strong>
                  game
                </span>
                <span>
                  <strong>{categories.length - 1}</strong>
                  nhóm
                </span>
                <span>
                  <strong>{gradeCount}</strong>
                  khối
                </span>
              </div>
            </div>

            <div className="catalog-controls">
              <label className="search-field">
                <span>Tìm</span>
                <input
                  type="search"
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder="Tên game, mã, khối lớp..."
                />
              </label>
              <div className="category-tabs" aria-label="Lọc theo nhóm game">
                {categories.map((category) => (
                  <button
                    className={category === selectedCategory ? "category-tab active" : "category-tab"}
                    key={category}
                    onClick={() => setSelectedCategory(category)}
                    type="button"
                  >
                    {category}
                  </button>
                ))}
              </div>
            </div>

            {isAdmin ? (
            <div className="library-actions">
              <label className="select-all-games">
                <input
                  type="checkbox"
                  checked={isLibrarySelectionMode && allVisibleGamesSelected}
                  disabled={!isLibrarySelectionMode || deletableFilteredGames.length === 0 || isDeletingGames}
                  onChange={toggleAllVisibleGames}
                />
                <span>Chọn tất cả đang hiển thị</span>
              </label>
              <span className="selection-count">{selectedDeletableGameCount} game đã chọn</span>
              <label className="selection-mode-switch">
                <span>Chế độ chọn</span>
                <input
                  checked={isLibrarySelectionMode}
                  disabled={isDeletingGames}
                  onChange={toggleLibrarySelectionMode}
                  type="checkbox"
                />
                <span className="switch-track" aria-hidden="true" />
              </label>
              <button
                className="delete-button"
                disabled={isDeletingGames || selectedDeletableGameCount === 0}
                onClick={deleteSelectedGames}
                type="button"
              >
                {isDeletingGames ? "Đang xóa..." : "Xóa đã chọn"}
              </button>
            </div>
            ) : null}

            <div
              className="game-list"
              ref={gameListRef}
              role="list"
              aria-label="Danh sách game đã cài"
              onScroll={(event) => {
                if (!shouldVirtualizeGameList) {
                  return;
                }
                const target = event.currentTarget;
                setGameListViewport({ height: target.clientHeight, scrollTop: target.scrollTop });
              }}
            >
              {filteredGames.length > 0 ? (
                <>
                  {virtualTopSpacer > 0 ? <div className="game-list-spacer" style={{ height: virtualTopSpacer }} /> : null}
                  {visibleGameRows.map((game, index) => {
                    const absoluteIndex = virtualStartIndex + index;
                  const isGameSelectedForDelete = selectedLibraryGameIds.includes(game.id);
                  const rowClassName = [
                    "game-row",
                    focusedGame?.id === game.id ? "selected" : "",
                    isAdmin && isLibrarySelectionMode ? "selection-mode" : "",
                    isGameSelectedForDelete ? "marked-for-delete" : "",
                  ].filter(Boolean).join(" ");

                  return (
                    <div
                      className={rowClassName}
                      key={game.id}
                      data-game-id={game.id}
                      onClick={() => selectLibraryGame(game.id)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          selectLibraryGame(game.id);
                        }
                      }}
                      role="listitem"
                      tabIndex={0}
                    >
                      {isAdmin && isLibrarySelectionMode ? (
                        <input
                          aria-label={"Chọn " + game.title}
                          checked={isGameSelectedForDelete}
                          className="game-select-checkbox"
                          disabled={game.isInstalled === false || isDeletingGames}
                          onChange={() => toggleLibraryGameSelection(game.id)}
                          onClick={(event) => event.stopPropagation()}
                          type="checkbox"
                        />
                      ) : (
                        <span className="game-selection-slot" aria-hidden="true" />
                      )}
                      <span className="game-rank">{String(absoluteIndex + 1).padStart(2, "0")}</span>
                      <span className="game-title-block">
                        <strong>{game.title}</strong>
                      </span>
                    </div>
                  );
                  })}
                  {virtualBottomSpacer > 0 ? <div className="game-list-spacer" style={{ height: virtualBottomSpacer }} /> : null}
                </>
              ) : (
                <div className="empty-state">
                  <strong>Không tìm thấy game phù hợp.</strong>
                  <span>Thử xoá bộ lọc hoặc cập nhật thêm game trong Cài đặt.</span>
                </div>
              )}
            </div>
          </div>

          <aside className="launch-panel" aria-label="Thao tác mở game">
            {focusedGame ? (
              <>
                <div className="selected-game-header">
                  <p className="eyebrow">Game đang chọn</p>
                  <h2>{focusedGame.title}</h2>
                  <span>{focusedGame.category} · {focusedGame.grade}</span>
                </div>
                <button
                  className="play-button"
                  disabled={busyGameId === focusedGame.id}
                  onClick={() => void playGame(focusedGame.id)}
                  type="button"
                >
                  {busyGameId === focusedGame.id ? "Đang mở..." : "Chơi ngay"}
                </button>
                <div className="launch-secondary">
                  <div className="game-detail-grid">
                    <span>
                      <strong>ID</strong>
                      {focusedGame.id}
                    </span>
                  </div>
                  <div className="deep-link-panel">
                    <div className="deep-link-heading">
                      <span>Deep link PowerPoint</span>
                      {isAdmin ? (
                        <button className="copy-link-button" onClick={() => void copySelectedDeepLink()} type="button">
                          Copy
                        </button>
                      ) : null}
                    </div>
                    <code>{selectedDeepLink}</code>
                  </div>
                </div>
              </>
            ) : (
              <div className="empty-state compact">
                <strong>Kho game đang trống.</strong>
                <span>Vào Cài đặt để quét folder hoặc USB chứa game.</span>
              </div>
            )}
          </aside>
        </section>
      ) : (
        <section className="settings-page">
          <div className="section-heading">
            <h2>Cài đặt</h2>
            <p>Thiết lập và cập nhật nguồn games cho app.</p>
          </div>

          <div className="settings-panel">
            <div>
              <p className="eyebrow">Cập nhật games</p>
              <h3>Đồng bộ games từ folder hoặc USB</h3>
              <p className="settings-copy">
                Chọn folder/USB chứa folder game, hoặc chọn trực tiếp nhiều file <code>.html</code>. Sau đó bấm Quét game để app so sánh
                với kho hiện tại; chỉ những game mới được chọn để đồng bộ khi bấm Xác nhận cập nhật.
              </p>
            </div>

            <div className="folder-picker">
              <input
                ref={folderInputRef}
                className="hidden-input"
                type="file"
                multiple
                onChange={(event) => void handleFolderSelected(event)}
              />
              <button onClick={() => void chooseSourceFolder()} type="button" className="secondary-button">
                Chọn folder / USB
              </button>
              <button onClick={() => void chooseSourceFiles()} type="button" className="secondary-button">
                Chọn file HTML
              </button>
              <button
                disabled={(pendingSourceDirs.length === 0 && pendingSourceFiles.length === 0 && sourceFilePaths.length === 0) || isScanningGames || isUpdatingGames}
                onClick={() => void scanGamesFromSelectedFolder()}
                type="button"
                className="scan-button"
              >
                {isScanningGames ? "Đang quét..." : "Quét game"}
              </button>
              <div className="selected-source">
                <span>Nguồn đã chọn</span>
                <strong>{selectedSourceName ?? "Chưa chọn"}</strong>
                <small>
                  {isScanningGames
                    ? "Đang phân tích folder game và file .html..."
                    : scanResult
                      ? scanResult.games.length + " game · " + scanNewCount + " game mới"
                      : pendingSourceDirs.length > 0
                        ? pendingSourceDirs.length + " folder nguồn · chờ quét game"
                        : sourceFilePaths.length > 0
                          ? sourceFilePaths.length + " file HTML · chờ quét game"
                          : pendingSourceFiles.length > 0
                            ? pendingSourceFiles.length + " file trong folder · chờ quét game"
                            : "Chọn folder hoặc file HTML"}
                </small>
              </div>
              <button
                disabled={!canUpdateScannedGames}
                onClick={() => void updateGamesFromSelectedFolder()}
                title={updateGamesButtonTitle}
                type="button"
              >
                {updateGamesButtonLabel}
              </button>
            </div>

            {scanResult ? (
              <div className="scan-results">
                <div className="scan-results-heading">
                  <div>
                    <p className="eyebrow">Kết quả quét</p>
                    <h3>Game mới để cập nhật</h3>
                  </div>
                  <span>{selectedGameIds.length}/{scanNewCount} game mới</span>
                </div>
                <div className="scan-list">
                  {scanResult.games.map((game) => (
                    <label className={game.isNew ? "scan-item" : "scan-item disabled"} key={game.id}>
                      <input
                        type="checkbox"
                        checked={selectedGameIds.includes(game.id)}
                        disabled={!game.isNew}
                        onChange={() => game.isNew && toggleGameSelection(game.id)}
                      />
                      <span className="scan-item-copy">
                        <strong>{game.title}</strong>
                        <small>
                          {game.sourceKind === "folder" ? "Folder game" : "HTML đơn"} · {game.entry} · {game.fileCount} file · {formatBytes(game.totalBytes)} · {game.sourcePath} · {game.id}
                        </small>
                      </span>
                      <span className={game.isNew ? "scan-badge new" : "scan-badge"}>
                        {game.isNew ? "Game mới" : "Đã có trong kho"}
                      </span>
                    </label>
                  ))}
                </div>
                {scanResult.skippedFiles > 0 ? (
                  <small className="scan-note">{scanResult.skippedFiles} mục không phải game hợp lệ đã được bỏ qua.</small>
                ) : null}
              </div>
            ) : null}

            {installSummary ? (
              <div className="update-summary">
                <strong>Đã đồng bộ {installSummary.installedGames} game mới</strong>
                <span>
                  {installSummary.installedGames} mục đã thêm vào kho, {installSummary.skippedFiles} mục bỏ qua.
                </span>
                <code>{installSummary.targetDir}</code>
              </div>
            ) : null}
          </div>

          <div className="settings-panel categories-panel">
            <div>
              <p className="eyebrow">Categories</p>
              <h3>Gom nhóm file game</h3>
              <p className="settings-copy">
                Tạo category và nhập các keywords, phân tách bằng dấu phẩy. Cấu hình này được lưu riêng trong app để dùng khi phân loại game.
              </p>
            </div>

            <form className="category-form" onSubmit={(event) => void saveCategory(event)}>
              <label className="category-field">
                <span>Tên Category</span>
                <input
                  maxLength={100}
                  onChange={(event) => setCategoryName(event.target.value)}
                  placeholder="Ví dụ: Toán lớp 4"
                  required
                  value={categoryName}
                />
              </label>
              <label className="category-field">
                <span>Keywords</span>
                <input
                  onChange={(event) => setCategoryKeywords(event.target.value)}
                  placeholder="Ví dụ: toán, cộng, trừ, lớp 4"
                  required
                  value={categoryKeywords}
                />
              </label>
              <div className="category-form-actions">
                <button className="category-save-button" disabled={isSavingCategory} type="submit">
                  {isSavingCategory ? "Đang lưu..." : editingCategoryId ? "Lưu thay đổi" : "Thêm Category"}
                </button>
                {editingCategoryId ? (
                  <button onClick={resetCategoryForm} type="button">Hủy sửa</button>
                ) : null}
              </div>
            </form>

            <div className="category-list" aria-label="Danh sách categories">
              {configuredCategories.length > 0 ? configuredCategories.map((category) => (
                <article className="category-item" key={category.id}>
                  <div className="category-item-copy">
                    <strong>{category.name}</strong>
                    <small>{category.id}</small>
                    <div className="category-keywords">
                      {category.keywords.map((keyword) => <span className="category-chip" key={keyword}>{keyword}</span>)}
                    </div>
                  </div>
                  <div className="category-item-actions">
                    <button className="category-edit-button" onClick={() => editCategory(category)} type="button">Sửa</button>
                    <button className="category-delete-button" onClick={() => requestDeleteCategory(category)} type="button">Xóa</button>
                  </div>
                </article>
              )) : (
                <div className="empty-state">
                  <strong>Chưa có category nào.</strong>
                  <span>Tạo category đầu tiên bằng form ở phía trên.</span>
                </div>
              )}
            </div>
          </div>
          <div className="settings-panel classify-panel">
            <div>
              <p className="eyebrow">Phân loại game</p>
              <h3>Tự động gán category theo keywords</h3>
              <p className="settings-copy">
                App sẽ so keywords của từng category với tên file HTML đã cài. Tất cả chữ tiếng Việt được quy về không dấu trước khi đối chiếu.
              </p>
            </div>

            <div className="classification-actions">
              <button
                className="classification-button"
                disabled={isClassifyingGames || configuredCategories.length === 0 || installedGameCount === 0}
                onClick={() => void classifyGamesByCategories()}
                type="button"
              >
                {isClassifyingGames ? "Đang phân loại..." : "Phân loại game"}
              </button>
              <div className="classification-status">
                <span>Nguồn phân loại</span>
                <strong>{configuredCategories.length} categories · {installedGameCount} game đã cài</strong>
                <small>
                  {configuredCategories.length === 0
                    ? "Tạo category và keywords trước khi phân loại."
                    : installedGameCount === 0
                      ? "Cập nhật game HTML vào app trước khi phân loại."
                      : "Sẵn sàng đối chiếu keywords với tên file game."}
                </small>
              </div>
            </div>

            {classificationSummary ? (
              <div className="classification-summary">
                <div className="classification-stats">
                  <span>
                    <strong>{classificationSummary.scannedGames}</strong>
                    Đã quét
                  </span>
                  <span>
                    <strong>{classificationSummary.matchedGames}</strong>
                    Khớp keywords
                  </span>
                  <span>
                    <strong>{classificationSummary.updatedGames}</strong>
                    Đã cập nhật
                  </span>
                  <span>
                    <strong>{classificationSummary.skippedGames}</strong>
                    Chưa khớp
                  </span>
                </div>

                {classificationSummary.games.length > 0 ? (
                  <div className="classification-list" aria-label="Kết quả phân loại game">
                    {classificationSummary.games.map((game) => (
                      <article className="classification-item" key={game.id}>
                        <div>
                          <strong>{game.title}</strong>
                          <small>{game.previousCategory} → {game.category}</small>
                        </div>
                        <span>{game.matchedKeywords.join(", ")}</span>
                      </article>
                    ))}
                  </div>
                ) : (
                  <div className="empty-state compact">
                    <strong>Chưa có game nào khớp keywords.</strong>
                    <span>Thêm keywords gần với tên file HTML rồi chạy lại.</span>
                  </div>
                )}
              </div>
            ) : null}
          </div>

          <div className="settings-panel import-panel">
            <div>
              <p className="eyebrow">Nhập kho game</p>
              <h3>Khôi phục game từ file zip</h3>
              <p className="settings-copy">
                Chọn file <code>.zip</code> có cấu trúc <code>games/&lt;game-id&gt;/...</code>. Game trùng ID sẽ được bỏ qua để không ghi đè kho hiện tại.
              </p>
            </div>

            <div className="import-archive-actions">
              <button
                className="archive-button"
                disabled={isImportingGames}
                onClick={() => void importGamesArchive()}
                type="button"
              >
                {isImportingGames ? "Đang nhập zip..." : "Nhập file zip"}
              </button>
              <div className="archive-status">
                <span>Quy tắc nhập</span>
                <strong>Không ghi đè game đã có</strong>
                <small>Import qua staging tạm; hoàn tất mới chuyển game vào kho.</small>
              </div>
            </div>

            {importSummary ? (
              <div className="update-summary archive-summary">
                <strong>Đã nhập {importSummary.importedGames} game</strong>
                <span>
                  {importSummary.importedFiles} file · bỏ qua {importSummary.skippedGames} game · {importSummary.skippedFiles} file
                </span>
                <code>{importSummary.archivePath}</code>
              </div>
            ) : null}
          </div>

          <div className="settings-panel archive-panel">
            <div>
              <p className="eyebrow">Xuất kho game</p>
              <h3>Lưu toàn bộ kho game thành file zip</h3>
              <p className="settings-copy">
                Tạo một file <code>.zip</code> chứa toàn bộ games đã cài và file categories nếu có. Nhập lại password admin để xác nhận trước khi xuất.
              </p>
            </div>

            <form className="archive-form" onSubmit={(event) => void exportGamesArchive(event)}>
              <label className="archive-field">
                <span>Password admin</span>
                <input
                  autoComplete="current-password"
                  onChange={(event) => setExportAdminPassword(event.target.value)}
                  placeholder="Nhập password admin để xuất zip"
                  required
                  type="password"
                  value={exportAdminPassword}
                />
              </label>
              <button
                className="archive-button"
                disabled={isExportingGames || installedGameCount === 0 || exportAdminPassword.length === 0}
                type="submit"
              >
                {isExportingGames ? "Đang xuất zip..." : "Xuất kho game"}
              </button>
              <div className="archive-status">
                <span>Kho hiện tại</span>
                <strong>{installedGameCount} game đã cài</strong>
                <small>
                  {isExportingGames && exportProgress
                    ? "Đã nén " + exportProgress.exportedFiles + " file"
                    : "File zip được tạo trong Downloads để lưu trữ khi cần."}
                </small>
              </div>
            </form>

            {exportSummary ? (
              <div className="update-summary archive-summary">
                <strong>Đã xuất {exportSummary.exportedGames} game</strong>
                <span>
                  {exportSummary.exportedFiles} file · {formatBytes(exportSummary.archiveBytes)}
                </span>
                <code>{exportSummary.archivePath}</code>
              </div>
            ) : null}
          </div>

          <div className="settings-panel diagnostics-panel">
            <div>
              <p className="eyebrow">Chẩn đoán app</p>
              <h3>Thông tin kho và cache</h3>
              <p className="settings-copy">
                Dùng phần này khi cần gửi trạng thái app để kiểm tra lỗi import/export, cache catalog hoặc dung lượng kho game.
              </p>
            </div>

            <div className="diagnostics-actions">
              <button
                className="secondary-button"
                disabled={isLoadingDiagnostics}
                onClick={() => void loadDiagnostics()}
                type="button"
              >
                {isLoadingDiagnostics ? "Đang đọc..." : "Tải diagnostics"}
              </button>
              <button
                className="secondary-button"
                disabled={!appDiagnostics}
                onClick={() => void copyDiagnostics()}
                type="button"
              >
                Copy diagnostics
              </button>
            </div>

            {appDiagnostics ? (
              <div className="diagnostics-grid">
                <span>
                  <small>Games</small>
                  <strong>{appDiagnostics.installedGames}</strong>
                </span>
                <span>
                  <small>Dung lượng</small>
                  <strong>{formatBytes(appDiagnostics.totalGameBytes)}</strong>
                </span>
                <span>
                  <small>Categories</small>
                  <strong>{appDiagnostics.categoriesCount}</strong>
                </span>
                <span>
                  <small>Catalog cache</small>
                  <strong>{appDiagnostics.catalogCacheExists ? appDiagnostics.catalogCacheValid ? "Hợp lệ" : "Hỏng" : "Chưa có"}</strong>
                </span>
                <code>{appDiagnostics.appDataDir}</code>
                <code>{appDiagnostics.gamesDir}</code>
              </div>
            ) : null}
          </div>

          <div className="settings-panel password-panel">
            <div>
              <p className="eyebrow">Tài khoản Admin</p>
              <h3>Đổi password</h3>
              <p className="settings-copy">
                Nhập password mới rồi lưu lại. App không yêu cầu password cũ cho thao tác này.
              </p>
            </div>

            <form className="password-form" onSubmit={(event) => void changeAdminPassword(event)}>
              <label className="password-field">
                <span>Password mới</span>
                <input
                  autoComplete="new-password"
                  minLength={4}
                  onChange={(event) => setNewAdminPassword(event.target.value)}
                  placeholder="Nhập password admin mới"
                  required
                  type="password"
                  value={newAdminPassword}
                />
              </label>
              <div className="password-form-actions">
                <button
                  className="category-save-button"
                  disabled={isChangingPassword || newAdminPassword.trim().length < 4}
                  type="submit"
                >
                  {isChangingPassword ? "Đang lưu..." : "Đổi password"}
                </button>
              </div>
            </form>
          </div>

        </section>
      )}

      <section className="exercise-view" aria-label="Khung xem bài tập">
        <div className="exercise-view-header">
          <div>
            <p className="eyebrow">Khung bài tập</p>
            <h2>{launchedGame ? launchedGame.title : focusedGame ? focusedGame.title : "Chưa mở bài tập"}</h2>
          </div>
          {launchedGame ? <span>{launchedGame.category} · {launchedGame.grade}</span> : null}
        </div>

        <div className={launchedGame ? "exercise-frame-shell active" : "exercise-frame-shell"}>
          {launchedGame ? (
            <iframe
              key={playerSrc ?? launchedGame.id}
              className="exercise-frame"
              onError={() => {
                appendDebug("game iframe error", { gameId: launchedGame.id, src: playerSrc });
                setError("Iframe game báo lỗi tải nội dung.");
                setStatus("Không tải được iframe game. Hãy copy log chẩn đoán gửi lại.");
              }}
              onLoad={() => {
                appendDebug("game iframe load", { gameId: launchedGame.id, src: playerSrc, isTauriRuntime });
              }}
              src={playerSrc ?? undefined}
              srcDoc={previewPlayerHtml}
              title={"Bài tập " + launchedGame.title}
            />
          ) : (
            <div className="exercise-placeholder">
              <p className="eyebrow">Sẵn sàng</p>
              <strong>{focusedGame ? focusedGame.title : "Chọn một bài tập"}</strong>
              <span>{focusedGame ? "Bấm Chơi ngay ở cột trái để mở trong khung này." : "Kho bài tập sẽ hiển thị ở cột trái."}</span>
            </div>
          )}
        </div>
      </section>
    </main>
  );
}

function App() {
  const [accountRole, setAccountRole] = useState<AccountRole | null>(null);

  if (!accountRole) {
    return <LoginScreen onLogin={setAccountRole} />;
  }

  return (
    <LauncherApp
      accountRole={accountRole}
      onLogout={() => setAccountRole(null)}
    />
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
