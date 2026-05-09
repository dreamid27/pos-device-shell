import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { check as checkUpdate } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import "./App.css";

const SERVICE_URL = "http://localhost:3333";

type Health = {
  status: string;
  service: string;
  version: string;
  subsystems: {
    printers: { ok: boolean; count: number };
    scales: { ok: boolean; count: number };
    rfid: { ok: boolean; count: number };
  };
};

type Printer = {
  name: string;
  protocol: string;
  connection: string;
  status: string;
};

function App() {
  const [health, setHealth] = useState<Health | null>(null);
  const [printers, setPrinters] = useState<Printer[]>([]);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const h = await fetch(`${SERVICE_URL}/health`).then((r) => r.json());
      setHealth(h);
      const p = await fetch(`${SERVICE_URL}/printers`).then((r) => r.json());
      setPrinters(p.printers ?? []);
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 4000);
    return () => clearInterval(id);
  }, [refresh]);

  useEffect(() => {
    isAutostartEnabled().then(setAutostart).catch(() => setAutostart(null));
  }, []);

  const toggleAutostart = async () => {
    try {
      if (autostart) {
        await disableAutostart();
        setAutostart(false);
      } else {
        await enableAutostart();
        setAutostart(true);
      }
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const hideToTray = async () => {
    try {
      await getCurrentWindow().hide();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const [updateMsg, setUpdateMsg] = useState<string | null>(null);

  const checkForUpdate = async () => {
    try {
      setUpdateMsg("Checking…");
      const update = await checkUpdate();
      if (!update) {
        setUpdateMsg("Up to date.");
        return;
      }
      setUpdateMsg(`Downloading ${update.version}…`);
      await update.downloadAndInstall();
      setUpdateMsg("Installed. Restarting…");
      await relaunch();
    } catch (e) {
      setUpdateMsg(null);
      setError((e as Error).message);
    }
  };

  return (
    <main className="container">
      <header>
        <h1>POS Device Service</h1>
        <span className={`status ${health ? "ok" : "down"}`}>
          {health ? health.status : "down"}
        </span>
      </header>

      <section className="card">
        <h2>Service</h2>
        <p>
          Endpoint: <code>{SERVICE_URL}</code>
        </p>
        <p>
          Version: <code>{health?.version ?? "—"}</code>
        </p>
        <p>
          Counts: printers <code>{health?.subsystems?.printers?.count ?? 0}</code>{" "}
          · scales <code>{health?.subsystems?.scales?.count ?? 0}</code> · rfid{" "}
          <code>{health?.subsystems?.rfid?.count ?? 0}</code>
        </p>
      </section>

      <section className="card">
        <h2>Printers ({printers.length})</h2>
        {printers.length === 0 ? (
          <p className="muted">No printers detected.</p>
        ) : (
          <ul className="devices">
            {printers.map((p) => (
              <li key={p.name}>
                <strong>{p.name}</strong>
                <span>
                  {p.protocol} · {p.connection} · {p.status}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="card">
        <h2>Startup</h2>
        <label>
          <input
            type="checkbox"
            checked={!!autostart}
            disabled={autostart === null}
            onChange={toggleAutostart}
          />{" "}
          Launch on system startup
        </label>
      </section>

      <section className="card row">
        <button onClick={refresh}>Refresh</button>
        <button onClick={hideToTray}>Hide to Tray</button>
        <button onClick={checkForUpdate}>Check Update</button>
      </section>

      {updateMsg && <p className="muted">{updateMsg}</p>}

      {error && <pre className="error">{error}</pre>}
    </main>
  );
}

export default App;
