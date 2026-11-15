// Цвет (тон) каждого звена цепочки «Вы → VPN → Прокси → Интернет» по состоянию (ТЗ §12).
import type { StartStep, Status } from '../ipc/types.ts'

export type LinkTone = 'ok' | 'pending' | 'bad' | 'idle' | 'blocked'

export interface ChainTones {
  you: LinkTone
  vpn: LinkTone
  proxy: LinkTone
  internet: LinkTone
}

export const STEP_ORDER: readonly StartStep[] = [
  'snapshot',
  'firewall',
  'xray',
  'check_vpn',
  'sing_box',
  'check_proxy',
]

function byHealth(h: Status['vpn']): LinkTone {
  return h === 'up' ? 'ok' : h === 'down' ? 'bad' : 'pending'
}

export function chainTones(s: Status): ChainTones {
  const c = s.chain
  const you: LinkTone = 'ok'
  switch (c.state) {
    case 'off':
      return { you, vpn: 'idle', proxy: 'idle', internet: 'idle' }
    case 'starting': {
      const i = STEP_ORDER.indexOf(c.step)
      return {
        you,
        vpn: i < 2 ? 'idle' : i < 4 ? 'pending' : 'ok',
        proxy: i < 4 ? 'idle' : 'pending',
        internet: 'idle',
      }
    }
    case 'protected':
      return { you, vpn: 'ok', proxy: 'ok', internet: 'ok' }
    case 'blocked': {
      const vpn = byHealth(s.vpn)
      const proxy: LinkTone = s.vpn === 'down' ? 'idle' : byHealth(s.proxy)
      return { you, vpn, proxy, internet: 'blocked' }
    }
    case 'stopping':
    case 'rolling_back':
      return { you, vpn: 'pending', proxy: 'pending', internet: 'idle' }
    case 'wifi_login':
      return { you, vpn: 'idle', proxy: 'idle', internet: c.real_ip_exposed ? 'bad' : 'pending' }
    case 'protection_disabled':
      return { you, vpn: byHealth(s.vpn), proxy: s.vpn === 'down' ? 'idle' : byHealth(s.proxy), internet: 'bad' }
  }
}
