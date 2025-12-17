import { useQuery } from '@tanstack/react-query'
import { fetchBots, type Bot } from '../lib/api'

export function useBots() {
  return useQuery<Bot[], Error>({
    queryKey: ['bots'],
    queryFn: fetchBots,
  })
}
