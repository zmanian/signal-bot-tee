const API_URL = import.meta.env.VITE_API_URL || 'https://cf847538c919958236086a69ad1a2281103c7fee-8081.dstack-pha-prod9.phala.network'

export interface Bot {
  username: string
  phone_number: string
  signal_link: string
  identity_key?: string
  registered_at: string
  model?: string
  description?: string
}

export interface Attestation {
  in_tee: boolean
  app_id?: string
  compose_hash?: string
  tdx_quote_base64?: string
  verification_url?: string
  report_data?: string
}

export async function fetchBots(): Promise<Bot[]> {
  const response = await fetch(`${API_URL}/v1/bots`)
  if (!response.ok) {
    throw new Error(`Failed to fetch bots: ${response.statusText}`)
  }
  // Backend returns array directly
  return response.json()
}

export async function fetchAttestation(): Promise<Attestation> {
  const response = await fetch(`${API_URL}/v1/attestation`)
  if (!response.ok) {
    throw new Error(`Failed to fetch attestation: ${response.statusText}`)
  }
  return response.json()
}

export async function fetchAttestationWithChallenge(challenge: string): Promise<Attestation> {
  const response = await fetch(`${API_URL}/v1/attestation?challenge=${encodeURIComponent(challenge)}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch attestation: ${response.statusText}`)
  }
  return response.json()
}
