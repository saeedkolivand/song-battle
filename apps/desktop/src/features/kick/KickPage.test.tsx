import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { KickOfficialStatus } from '@sb/types';
import { KickPage } from './KickPage';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);
const mockOpenUrl = vi.mocked(openUrl);

// Capture the `kick-auth` subscriber so a test can fire it like the real
// loopback server would once the OAuth callback lands.
let kickAuthHandler: (() => void) | undefined;

function statusResponse(over: Partial<KickOfficialStatus> = {}): KickOfficialStatus {
  return { authorized: false, subscriptionActive: false, ...over };
}

beforeEach(() => {
  kickAuthHandler = undefined;
  mockInvoke.mockReset();
  mockOpenUrl.mockReset().mockResolvedValue();
  mockListen.mockReset().mockImplementation((event, handler) => {
    kickAuthHandler = () => handler({ event, id: 0, payload: undefined });
    return Promise.resolve(() => {});
  });
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'kick_official_status') return Promise.resolve(statusResponse());
    return Promise.resolve(undefined);
  });
});

describe('KickPage — Official/Unofficial toggle', () => {
  it('defaults to the Unofficial panel and switches to Official on toggle', async () => {
    const user = userEvent.setup();
    render(<KickPage />);

    // "Channel" has a visible hint sibling inside the same <label>, which folds
    // into its accessible name — assert via the input's placeholder instead.
    expect(screen.getByPlaceholderText('channel-name')).toBeInTheDocument();
    expect(screen.queryByLabelText('Client ID')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Unofficial' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(screen.getByRole('button', { name: 'Official' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );

    await user.click(screen.getByRole('button', { name: 'Official' }));

    expect(screen.queryByPlaceholderText('channel-name')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Client ID')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Official' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });
});

describe('KickPage — Official login', () => {
  it('disables Login with Kick until both Client ID and Secret are filled, then starts OAuth and opens the URL', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'kick_official_status') return Promise.resolve(statusResponse());
      if (cmd === 'kick_oauth_start')
        return Promise.resolve('https://id.kick.com/oauth/authorize?state=abc');
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup();
    render(<KickPage />);
    await user.click(screen.getByRole('button', { name: 'Official' }));

    const loginBtn = screen.getByRole('button', { name: /Login with Kick/ });
    expect(loginBtn).toBeDisabled();

    await user.type(screen.getByLabelText('Client ID'), '  my-id  ');
    expect(loginBtn).toBeDisabled(); // secret still empty
    await user.type(screen.getByLabelText('Client Secret'), 'my-secret');
    expect(loginBtn).toBeEnabled();

    await user.click(loginBtn);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('kick_oauth_start', {
        clientId: 'my-id',
        clientSecret: 'my-secret',
      }),
    );
    expect(mockOpenUrl).toHaveBeenCalledWith('https://id.kick.com/oauth/authorize?state=abc');
  });

  it('refreshes to authorized when the kick-auth event fires, and Disconnect clears it back', async () => {
    let authorized = false;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'kick_official_status') return Promise.resolve(statusResponse({ authorized }));
      if (cmd === 'kick_official_disconnect') {
        authorized = false;
        return Promise.resolve(undefined);
      }
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup();
    render(<KickPage />);
    await user.click(screen.getByRole('button', { name: 'Official' }));

    expect(await screen.findByText('not connected')).toBeInTheDocument();

    // Simulate the OAuth callback completing on the Rust side.
    authorized = true;
    kickAuthHandler?.();

    expect(await screen.findByText('authorized')).toBeInTheDocument();
    const disconnectBtn = screen.getByRole('button', { name: 'Disconnect' });
    expect(disconnectBtn).toBeEnabled();

    await user.click(disconnectBtn);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('kick_official_disconnect'));
    expect(await screen.findByText('not connected')).toBeInTheDocument();
  });

  it('surfaces a status-fetch failure via the error note', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'kick_official_status') return Promise.reject(new Error('backend unreachable'));
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup();
    render(<KickPage />);
    await user.click(screen.getByRole('button', { name: 'Official' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('backend unreachable');
  });
});
