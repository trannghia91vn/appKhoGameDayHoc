import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

type BuilderMessage = {
  source: "yeutre-vocab-builder";
  type: "export";
  html: string;
  fileName: string;
  title: string;
};

export default function GameBuilder({
  onStatus,
  onExported,
}: {
  onStatus: (message: string, error?: string | null) => void;
  onExported: (gameId: string) => void;
}) {
  const frameRef = useRef<HTMLIFrameElement | null>(null);
  const [isExporting, setIsExporting] = useState(false);

  useEffect(() => {
    async function receive(event: MessageEvent<BuilderMessage>) {
      if (event.source !== frameRef.current?.contentWindow) return;
      if (event.data?.source !== "yeutre-vocab-builder" || event.data.type !== "export") return;
      try {
        setIsExporting(true);
        onStatus("Đang xuất game vào Kho game...");
        const html = Array.from(new TextEncoder().encode(event.data.html));
        const gameId = await invoke<string>("upsert_vocab_builder_game", {
          title: event.data.title || "Kho từ vựng",
          html,
        });
        onStatus("Đã cập nhật game từ vựng trong Kho game.");
        onExported(gameId);
      } catch (error) {
        onStatus("Không xuất được game.", String(error));
      } finally {
        setIsExporting(false);
      }
    }
    window.addEventListener("message", receive);
    return () => window.removeEventListener("message", receive);
  }, [onExported, onStatus]);

  return (
    <section className="html-builder-shell" aria-busy={isExporting}>
      {isExporting ? <div className="html-builder-progress">Đang xuất game vào Kho game...</div> : null}
      <iframe ref={frameRef} src="/game-builder.html" title="Làm game từ vựng" />
    </section>
  );
}
