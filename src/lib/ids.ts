let counter = 0

/** 生成规则 ID：优先 crypto.randomUUID，回落到时间戳+计数器 */
export function newId(prefix = 'rule'): string {
  const c = globalThis.crypto
  if (c && typeof c.randomUUID === 'function') {
    return `${prefix}_${c.randomUUID().replace(/-/g, '').slice(0, 10)}`
  }
  counter += 1
  return `${prefix}_${Date.now().toString(36)}${counter.toString(36)}`
}
