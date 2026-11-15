import { describe, expect, it } from 'vitest'
import { endpointKey, isValidIPv6, parseProxyLine, parseProxyList, reorder } from './proxyParse.ts'

const ok = (s: string) => {
  const r = parseProxyLine(s)
  if (!r.ok) throw new Error(`ожидался успех, получено: ${r.error}`)
  return r.endpoint
}
const bad = (s: string) => {
  const r = parseProxyLine(s)
  return r.ok ? null : r.error
}

describe('parseProxyLine: корректные строки', () => {
  it('socks5 с логином и паролем', () => {
    expect(ok('socks5://USER:PASS@1.2.3.4:8000')).toEqual({
      scheme: 'socks5', host: '1.2.3.4', port: 8000, username: 'USER', password: 'PASS',
    })
  })
  it('http без авторизации, схема и хост в любом регистре', () => {
    expect(ok('HTTP://Proxy.Example.COM:3128')).toEqual({
      scheme: 'http', host: 'proxy.example.com', port: 3128, username: null, password: null,
    })
  })
  it('только логин без пароля', () => {
    expect(ok('socks5://user@203.0.113.5:1080')).toMatchObject({ username: 'user', password: null })
  })
  it('пустой пароль считается отсутствующим', () => {
    expect(ok('socks5://user:@203.0.113.5:1080').password).toBeNull()
  })
  it('IPv6 в квадратных скобках — хост без скобок', () => {
    expect(ok('socks5://u:p@[2001:db8::1]:1080')).toMatchObject({ host: '2001:db8::1', port: 1080 })
    expect(ok('http://[::1]:8080').host).toBe('::1')
    expect(ok('http://[::ffff:192.0.2.1]:8080').host).toBe('::ffff:192.0.2.1')
  })
  it('percent-encoding в логине и пароле', () => {
    const e = ok('socks5://us%40er:p%40ss%3Aw%2Frd%25@host.example.com:1080')
    expect(e.username).toBe('us@er')
    expect(e.password).toBe('p@ss:w/rd%')
  })
  it('юникод в пароле через percent-encoding', () => {
    expect(ok('http://u:%D0%BF%D0%B0%D1%80@h.example.com:80').password).toBe('пар')
  })
  it('необработанный @ в пароле: разделитель — последний @', () => {
    expect(ok('socks5://u:p@ss@1.2.3.4:1080')).toMatchObject({ username: 'u', password: 'p@ss', host: '1.2.3.4' })
  })
  it('допускает завершающий слэш и пробелы по краям', () => {
    expect(ok('  socks5://1.2.3.4:1080/  ').port).toBe(1080)
  })
  it('крайние значения порта', () => {
    expect(ok('http://1.2.3.4:1').port).toBe(1)
    expect(ok('http://1.2.3.4:65535').port).toBe(65535)
  })
})

describe('parseProxyLine: мусор', () => {
  it('пустое', () => {
    expect(bad('')).toBe('empty')
    expect(bad('   ')).toBe('empty')
  })
  it('схема', () => {
    expect(bad('1.2.3.4:1080')).toBe('scheme')
    expect(bad('https://1.2.3.4:1080')).toBe('scheme')
    expect(bad('ftp://1.2.3.4:21')).toBe('scheme')
    expect(bad('hello world')).toBe('whitespace')
    expect(bad('мусор')).toBe('scheme')
  })
  it('пробелы и управляющие символы внутри', () => {
    expect(bad('socks5://u:p ass@1.2.3.4:1080')).toBe('whitespace')
    expect(bad('socks5://1.2.3.4:1080\u0000')).toBe('whitespace')
  })
  it('порт', () => {
    expect(bad('socks5://1.2.3.4')).toBe('port')
    expect(bad('socks5://1.2.3.4:')).toBe('port')
    expect(bad('socks5://1.2.3.4:0')).toBe('port')
    expect(bad('socks5://1.2.3.4:65536')).toBe('port')
    expect(bad('socks5://1.2.3.4:80a')).toBe('port')
    expect(bad('socks5://1.2.3.4:-1')).toBe('port')
    expect(bad('socks5://[2001:db8::1]')).toBe('port')
  })
  it('хост', () => {
    expect(bad('socks5://:1080')).toBe('host')
    expect(bad('socks5://999.1.1.1:1080')).toBe('host')
    expect(bad('socks5://1.2.3:1080')).toBe('host')
    expect(bad('socks5://-bad.example.com:1080')).toBe('host')
    expect(bad('socks5://exa_mple.com:1080')).toBe('host')
    expect(bad('socks5://a..b:1080')).toBe('host')
  })
  it('IPv6 без скобок и неверный IPv6', () => {
    expect(bad('socks5://2001:db8::1:1080')).toBe('ipv6_brackets')
    expect(bad('socks5://[2001:db8::1:1080')).toBe('ipv6_invalid')
    expect(bad('socks5://[2001:db8:::1]:1080')).toBe('ipv6_invalid')
    expect(bad('socks5://[gggg::1]:1080')).toBe('ipv6_invalid')
    expect(bad('socks5://[1:2:3:4:5:6:7:8:9]:1080')).toBe('ipv6_invalid')
  })
  it('путь и параметры', () => {
    expect(bad('socks5://1.2.3.4:1080/path')).toBe('path')
    expect(bad('socks5://1.2.3.4:1080?x=1')).toBe('path')
    expect(bad('socks5://1.2.3.4:1080#frag')).toBe('path')
  })
  it('логин пуст, битое percent-encoding', () => {
    expect(bad('socks5://:pass@1.2.3.4:1080')).toBe('userinfo')
    expect(bad('socks5://u:%ZZ@1.2.3.4:1080')).toBe('encoding')
    expect(bad('socks5://u:%E0%A4%A@1.2.3.4:1080')).toBe('encoding')
  })
  it('никогда не бросает исключений на произвольном вводе', () => {
    const junk = ['://', 'socks5://', 'socks5://@', 'socks5://@:', '[', ']', 'http://[]:1', '\u0000', '%', 'http://%:%@%:%']
    for (const j of junk) expect(() => parseProxyLine(j)).not.toThrow()
  })
})

describe('isValidIPv6', () => {
  it('допустимые', () => {
    for (const s of ['::', '::1', '1::', '2001:db8::1', '1:2:3:4:5:6:7:8', '::ffff:1.2.3.4']) {
      expect(isValidIPv6(s), s).toBe(true)
    }
  })
  it('недопустимые', () => {
    for (const s of ['1', ':::', '1:2:3:4:5:6:7', '1:2:3:4:5:6:7:8:9', '12345::1', '1::2::3', '::1.2.3']) {
      expect(isValidIPv6(s), s).toBe(false)
    }
  })
})

describe('parseProxyList', () => {
  it('пропускает пустые строки и комментарии, нумерует строки', () => {
    const list = parseProxyList('# заметка\r\nsocks5://1.2.3.4:1080\n\nмусор\nhttp://5.6.7.8:80')
    expect(list.map((x) => x.line)).toEqual([2, 4, 5])
    expect(list.map((x) => x.result.ok)).toEqual([true, false, true])
  })
})

describe('вспомогательные', () => {
  it('endpointKey приводит регистр и оборачивает IPv6', () => {
    expect(endpointKey('socks5', 'Host.Example.com', 1080)).toBe('socks5://host.example.com:1080')
    expect(endpointKey('http', '::1', 80)).toBe('http://[::1]:80')
  })
  it('reorder', () => {
    expect(reorder(['a', 'b', 'c'], 2, 0)).toEqual(['c', 'a', 'b'])
    expect(reorder(['a', 'b', 'c'], 0, 1)).toEqual(['b', 'a', 'c'])
    expect(reorder(['a', 'b', 'c'], 0, 99)).toEqual(['b', 'c', 'a'])
    expect(reorder(['a', 'b'], 5, 0)).toEqual(['a', 'b'])
  })
})
