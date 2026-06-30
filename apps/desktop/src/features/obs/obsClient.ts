import OBSWebSocket from 'obs-websocket-js';
import type { ConnectionState } from '@sb/types';

// Thin wrapper over OBSWebSocket (request/response, low throughput). The dashboard
// talks to OBS 28+ directly over its built-in WebSocket 5.x server. Status is surfaced
// through a single listener so the Zustand store can mirror it as a ConnectionState.
type StatusListener = (state: ConnectionState, error?: string) => void;

function sceneName(scene: unknown): string {
  return typeof scene === 'object' && scene !== null && 'sceneName' in scene
    ? String((scene as { sceneName: unknown }).sceneName ?? '')
    : '';
}

class ObsClient {
  private readonly obs = new OBSWebSocket();
  private listener: StatusListener | null = null;

  constructor() {
    this.obs.on('Identified', () => this.emit('connected'));
    this.obs.on('ConnectionClosed', () => this.emit('disconnected'));
    this.obs.on('ConnectionError', (err) => this.emit('error', err?.message ?? String(err)));
  }

  onStatus(listener: StatusListener): void {
    this.listener = listener;
  }

  private emit(state: ConnectionState, error?: string): void {
    this.listener?.(state, error);
  }

  async connect(url: string, password: string): Promise<void> {
    this.emit('connecting');
    try {
      // 'connected' is emitted by the 'Identified' listener (which fires before
      // connect() resolves); don't re-emit here or the scene list refreshes twice.
      await this.obs.connect(url, password || undefined);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      this.emit('error', message);
      throw new Error(message);
    }
  }

  async disconnect(): Promise<void> {
    await this.obs.disconnect();
    this.emit('disconnected');
  }

  async switchScene(name: string): Promise<void> {
    await this.obs.call('SetCurrentProgramScene', { sceneName: name });
  }

  async setBrowserSourceUrl(inputName: string, url: string): Promise<void> {
    await this.obs.call('SetInputSettings', { inputName, inputSettings: { url }, overlay: true });
  }

  async listScenes(): Promise<string[]> {
    const res = await this.obs.call('GetSceneList');
    return res.scenes.map(sceneName).filter((n) => n.length > 0);
  }
}

// Module singleton — survives page navigation so the connection persists.
export const obsClient = new ObsClient();
