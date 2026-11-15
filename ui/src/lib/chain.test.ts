import { describe, expect, it } from 'vitest'
import { chainTones } from './chain.ts'
import type { Status } from '../ipc/types.ts'

const base: Status = {
  chain: { state: 'off' },
  vpn: 'unknown',
  proxy: 'unknown',
  kill_switch_active: false,
  exit: null,
  active_profile: null,
  active_proxy_id: null,
  changed: [],
  pending_restore: false,
  conflicts: [],
  last_error: null,
}

describe('chainTones', () => {
  it('защищено — все звенья зелёные', () => {
    expect(chainTones({ ...base, chain: { state: 'protected' } })).toEqual({
      you: 'ok', vpn: 'ok', proxy: 'ok', internet: 'ok',
    })
  })
  it('блок из-за прокси: VPN зелёный, прокси красный, интернет заблокирован', () => {
    const t = chainTones({ ...base, chain: { state: 'blocked', reason: 'proxy_down' }, vpn: 'up', proxy: 'down' })
    expect(t).toMatchObject({ vpn: 'ok', proxy: 'bad', internet: 'blocked' })
  })
  it('VPN упал — прокси не считается упавшим', () => {
    const t = chainTones({ ...base, chain: { state: 'blocked', reason: 'vpn_down' }, vpn: 'down', proxy: 'down' })
    expect(t).toMatchObject({ vpn: 'bad', proxy: 'idle' })
  })
  it('запуск: на шаге проверки прокси VPN уже зелёный', () => {
    const t = chainTones({ ...base, chain: { state: 'starting', step: 'check_proxy' } })
    expect(t).toMatchObject({ vpn: 'ok', proxy: 'pending' })
  })
})
