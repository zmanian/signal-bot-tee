import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Check, ChevronDown, Copy, ExternalLink, RefreshCw, AlertCircle } from 'lucide-react'
import { useAttestation, useAttestationWithChallenge } from '../hooks/useAttestation'

export function VerificationPanel() {
  const { data: attestation, isLoading, error, refetch } = useAttestation()
  const [showTechnical, setShowTechnical] = useState(false)
  const [challenge, setChallenge] = useState('')
  const [submittedChallenge, setSubmittedChallenge] = useState<string | null>(null)
  const [copied, setCopied] = useState<string | null>(null)

  const { data: challengeAttestation, isLoading: challengeLoading } = useAttestationWithChallenge(submittedChallenge)

  const displayAttestation = submittedChallenge ? challengeAttestation : attestation

  const copyToClipboard = async (text: string, key: string) => {
    await navigator.clipboard.writeText(text)
    setCopied(key)
    setTimeout(() => setCopied(null), 2000)
  }

  const handleChallengeSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (challenge.trim()) {
      setSubmittedChallenge(challenge.trim())
    }
  }

  if (error) {
    return (
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        className="glass-card p-8 text-center"
      >
        <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-red-500/10 flex items-center justify-center">
          <AlertCircle className="w-8 h-8 text-red-400" />
        </div>
        <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-2">TEE Unavailable</h3>
        <p className="text-[var(--text-muted)] mb-6">Could not connect to the attestation service.</p>
        <button onClick={() => refetch()} className="glass-button flex items-center gap-2 mx-auto">
          <RefreshCw className="w-4 h-4" />
          Retry
        </button>
      </motion.div>
    )
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="glass-card p-8 md:p-10"
    >
      <h2 className="text-2xl font-bold text-[var(--text-primary)] mb-8">Security Verification</h2>

      {/* Simple trust indicators */}
      <div className="space-y-4 mb-10">
        <TrustIndicator
          loading={isLoading}
          verified={displayAttestation?.in_tee}
          label="Running in secure hardware (Intel TDX)"
          description="Your messages are processed in encrypted memory that even the server operator cannot read"
        />
        <TrustIndicator
          loading={isLoading}
          verified={!!displayAttestation?.compose_hash}
          label="Code verified by Phala Network"
          description="The exact software running has been cryptographically attested"
        />
        <TrustIndicator
          loading={isLoading}
          verified={true}
          label="Signal end-to-end encryption"
          description="Messages are encrypted on your device before reaching the server"
        />
      </div>

      {/* Technical details toggle */}
      <button
        onClick={() => setShowTechnical(!showTechnical)}
        className="w-full glass-button flex items-center justify-center gap-2 mb-4 py-4"
      >
        View Full Attestation
        <motion.div
          animate={{ rotate: showTechnical ? 180 : 0 }}
          transition={{ duration: 0.3 }}
        >
          <ChevronDown className="w-4 h-4" />
        </motion.div>
      </button>

      <AnimatePresence>
        {showTechnical && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.3 }}
            className="overflow-hidden"
          >
            <div className="space-y-8 pt-6 border-t border-white/5">
              {/* Interactive challenge verification */}
              <div>
                <h4 className="font-medium text-[var(--text-primary)] mb-3">Liveness Verification</h4>
                <p className="text-sm text-[var(--text-muted)] mb-3">
                  Submit your own challenge to prove this attestation is fresh:
                </p>
                <form onSubmit={handleChallengeSubmit} className="flex gap-2">
                  <input
                    type="text"
                    value={challenge}
                    onChange={(e) => setChallenge(e.target.value)}
                    placeholder="Enter a random nonce..."
                    className="flex-1 px-4 py-3 rounded-xl bg-white/5 border border-white/10 focus:outline-none focus:border-[var(--accent-start)] transition-colors text-sm"
                  />
                  <button
                    type="submit"
                    disabled={challengeLoading || !challenge.trim()}
                    className="glass-button glass-button-primary disabled:opacity-50"
                  >
                    {challengeLoading ? (
                      <RefreshCw className="w-4 h-4 animate-spin" />
                    ) : (
                      'Verify'
                    )}
                  </button>
                </form>
                {submittedChallenge && challengeAttestation && (
                  <p className="text-sm text-emerald-400 mt-2 flex items-center gap-1">
                    <Check className="w-4 h-4" />
                    Challenge "{submittedChallenge}" embedded in attestation
                  </p>
                )}
              </div>

              {/* App ID */}
              {displayAttestation?.app_id && (
                <DataRow
                  label="App ID"
                  value={displayAttestation.app_id}
                  copyable
                  copied={copied === 'app_id'}
                  onCopy={() => copyToClipboard(displayAttestation.app_id!, 'app_id')}
                />
              )}

              {/* Compose Hash */}
              {displayAttestation?.compose_hash && (
                <DataRow
                  label="Compose Hash"
                  value={displayAttestation.compose_hash}
                  copyable
                  copied={copied === 'compose_hash'}
                  onCopy={() => copyToClipboard(displayAttestation.compose_hash!, 'compose_hash')}
                  link="https://github.com/zmanian/signal-bot-tee/blob/main/docker/phala-compose.yaml"
                  linkLabel="View Source"
                />
              )}

              {/* TDX Quote */}
              {displayAttestation?.tdx_quote_base64 && (
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-medium text-[var(--text-secondary)]">TDX Quote (Base64)</span>
                    <button
                      onClick={() => copyToClipboard(displayAttestation.tdx_quote_base64!, 'tdx_quote')}
                      className="text-sm text-[var(--accent-start)] hover:text-[var(--accent-end)] flex items-center gap-1 transition-colors"
                    >
                      {copied === 'tdx_quote' ? (
                        <>
                          <Check className="w-4 h-4" />
                          Copied!
                        </>
                      ) : (
                        <>
                          <Copy className="w-4 h-4" />
                          Copy
                        </>
                      )}
                    </button>
                  </div>
                  <div className="code-block text-xs max-h-32 overflow-y-auto">
                    {displayAttestation.tdx_quote_base64}
                  </div>
                </div>
              )}

              {/* Verify on Phala */}
              {displayAttestation?.verification_url && (
                <a
                  href={displayAttestation.verification_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="glass-button glass-button-primary flex items-center justify-center gap-2 w-full"
                >
                  <ExternalLink className="w-4 h-4" />
                  Verify on Phala Portal
                </a>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  )
}

function TrustIndicator({
  loading,
  verified,
  label,
  description,
}: {
  loading: boolean
  verified?: boolean
  label: string
  description: string
}) {
  return (
    <motion.div
      initial={{ opacity: 0, x: -20 }}
      animate={{ opacity: 1, x: 0 }}
      className="trust-indicator p-4 md:p-5"
    >
      <div className="check-icon">
        {loading ? (
          <RefreshCw className="w-4 h-4 animate-spin" />
        ) : verified ? (
          <Check className="w-4 h-4 check-animated" />
        ) : (
          <AlertCircle className="w-4 h-4" />
        )}
      </div>
      <div>
        <p className="font-medium text-[var(--text-primary)]">{label}</p>
        <p className="text-sm text-[var(--text-muted)]">{description}</p>
      </div>
    </motion.div>
  )
}

function DataRow({
  label,
  value,
  copyable,
  copied,
  onCopy,
  link,
  linkLabel,
}: {
  label: string
  value: string
  copyable?: boolean
  copied?: boolean
  onCopy?: () => void
  link?: string
  linkLabel?: string
}) {
  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-medium text-[var(--text-secondary)]">{label}</span>
        <div className="flex items-center gap-3">
          {link && (
            <a
              href={link}
              target="_blank"
              rel="noopener noreferrer"
              className="text-sm text-[var(--accent-start)] hover:text-[var(--accent-end)] flex items-center gap-1 transition-colors"
            >
              <ExternalLink className="w-3 h-3" />
              {linkLabel}
            </a>
          )}
          {copyable && onCopy && (
            <button
              onClick={onCopy}
              className="text-sm text-[var(--accent-start)] hover:text-[var(--accent-end)] flex items-center gap-1 transition-colors"
            >
              {copied ? (
                <>
                  <Check className="w-4 h-4" />
                  Copied!
                </>
              ) : (
                <>
                  <Copy className="w-4 h-4" />
                  Copy
                </>
              )}
            </button>
          )}
        </div>
      </div>
      <div className="code-block text-xs">{value}</div>
    </div>
  )
}

export function VerificationPanelSkeleton() {
  return (
    <div className="glass-card p-6 md:p-8">
      <div className="h-8 w-48 skeleton mb-6" />
      <div className="space-y-3 mb-8">
        {[1, 2, 3].map((i) => (
          <div key={i} className="h-16 skeleton rounded-xl" />
        ))}
      </div>
      <div className="h-12 skeleton rounded-2xl" />
    </div>
  )
}
