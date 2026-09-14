import type { PlatformTemplate } from '@/types'

/**
 * 常用游戏平台进程模板。
 * 注意：模板只是常见进程名的预填清单，不检测本机是否安装。
 */
export const PLATFORM_TEMPLATES: PlatformTemplate[] = [
  {
    id: 'steam',
    name: 'Steam 平台',
    description: 'Steam 客户端与启动器',
    processes: [
      { name: 'steam', path: '' },
      { name: 'steamwebhelper', path: '' },
    ],
  },
  {
    id: 'epic',
    name: 'Epic Games',
    description: 'Epic 启动器',
    processes: [{ name: 'EpicGamesLauncher', path: '' }],
  },
  {
    id: 'wegame',
    name: 'WeGame',
    description: '腾讯 WeGame',
    processes: [{ name: 'wegame', path: '' }, { name: 'rail_tool', path: '' }],
  },
  {
    id: 'battle',
    name: '暴雪 Battle.net',
    description: 'Battle.net 启动器',
    processes: [{ name: 'Battle.net', path: '' }, { name: 'Agent', path: '' }],
  },
  {
    id: 'ubisoft',
    name: 'Ubisoft Connect',
    description: '育碧启动器',
    processes: [{ name: 'UbisoftConnect', path: '' }, { name: 'upc', path: '' }],
  },
  {
    id: 'riot',
    name: 'Riot 客户端',
    description: '英雄联盟 / Valorant 启动器',
    processes: [{ name: 'RiotClientServices', path: '' }, { name: 'LeagueClient', path: '' }],
  },
  {
    id: 'xbox',
    name: 'Xbox 应用',
    description: '微软商店 Xbox / Game Bar',
    processes: [{ name: 'XboxPwa', path: '' }, { name: 'GameBar', path: '' }],
  },
  {
    id: 'netease',
    name: '网易官方平台',
    description: '网易游戏平台',
    processes: [{ name: 'launcher', path: '' }],
  },
]
