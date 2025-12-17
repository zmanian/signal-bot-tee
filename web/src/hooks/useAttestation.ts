import { useQuery } from '@tanstack/react-query'
import { fetchAttestation, fetchAttestationWithChallenge, type Attestation } from '../lib/api'

export function useAttestation() {
  return useQuery<Attestation, Error>({
    queryKey: ['attestation'],
    queryFn: fetchAttestation,
  })
}

export function useAttestationWithChallenge(challenge: string | null) {
  return useQuery<Attestation, Error>({
    queryKey: ['attestation', challenge],
    queryFn: () => fetchAttestationWithChallenge(challenge!),
    enabled: !!challenge,
  })
}
