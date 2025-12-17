import { BotCard, BotCardSkeleton } from './components/BotCard'
import { VerificationPanel, VerificationPanelSkeleton } from './components/VerificationPanel'
import { useBots } from './hooks/useBots'
import { useAttestation } from './hooks/useAttestation'
import { motion } from 'framer-motion'
import { AlertCircle, RefreshCw, Shield, Lock, Cpu } from 'lucide-react'

function App() {
  const { data: bots, isLoading: botsLoading, error: botsError, refetch: refetchBots } = useBots()
  const { isLoading: attestationLoading } = useAttestation()

  return (
    <div className="min-h-screen">
      {/* Floating particles */}
      <div className="particle" />
      <div className="particle" />
      <div className="particle" />

      {/* Hero Section */}
      <section className="py-16 md:py-24">
        <div className="w-full max-w-4xl mx-auto px-8 md:px-12 text-center">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
          >
            <div className="inline-flex items-center gap-2 px-3 py-1.5 mb-6 rounded-full glass-card text-xs font-medium">
              <span className="w-1.5 h-1.5 bg-emerald-500 rounded-full animate-pulse" />
              <span className="text-[var(--text-secondary)]">Secured by Intel TDX</span>
            </div>

            <h1 className="text-4xl md:text-6xl font-bold mb-4 leading-tight">
              <span className="gradient-text">Private AI</span>
              <span className="text-[var(--text-primary)]"> Conversations</span>
            </h1>

            <p className="text-lg md:text-xl text-[var(--text-secondary)] mb-8 max-w-xl mx-auto">
              Chat with AI through Signal, protected by hardware security.
            </p>

            <div className="flex flex-wrap justify-center gap-6 text-sm text-[var(--text-muted)]">
              <div className="flex items-center gap-2">
                <Shield className="w-4 h-4 text-[var(--accent-start)]" />
                <span>End-to-End Encrypted</span>
              </div>
              <div className="flex items-center gap-2">
                <Cpu className="w-4 h-4 text-[var(--accent-mid)]" />
                <span>TEE Protected</span>
              </div>
              <div className="flex items-center gap-2">
                <Lock className="w-4 h-4 text-[var(--accent-end)]" />
                <span>Verifiable</span>
              </div>
            </div>
          </motion.div>
        </div>
      </section>

      {/* Bot Cards Section */}
      <section className="pb-16 md:pb-20">
        <div className="w-full max-w-xl mx-auto px-8 md:px-12">
          {botsError ? (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-8 text-center"
            >
              <div className="w-12 h-12 mx-auto mb-3 rounded-full bg-amber-500/10 flex items-center justify-center">
                <AlertCircle className="w-6 h-6 text-amber-400" />
              </div>
              <h3 className="text-lg font-semibold text-[var(--text-primary)] mb-2">
                Could not load bots
              </h3>
              <p className="text-[var(--text-muted)] text-sm mb-4">
                The bot directory is temporarily unavailable.
              </p>
              <button
                onClick={() => refetchBots()}
                className="glass-button text-sm flex items-center gap-2 mx-auto"
              >
                <RefreshCw className="w-3 h-3" />
                Retry
              </button>
            </motion.div>
          ) : botsLoading ? (
            <BotCardSkeleton />
          ) : bots && bots.length > 0 ? (
            <div className="space-y-6">
              {bots.map((bot, index) => (
                <BotCard key={bot.username} bot={bot} index={index} featured={bots.length === 1} />
              ))}
            </div>
          ) : (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-6 text-center"
            >
              <p className="text-[var(--text-muted)] text-sm">No bots registered yet.</p>
            </motion.div>
          )}
        </div>
      </section>

      {/* Verification Section */}
      <section id="verify" className="py-16 md:py-20 border-t border-white/5">
        <div className="w-full max-w-2xl mx-auto px-8 md:px-12">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
            className="text-center mb-8"
          >
            <h2 className="text-2xl md:text-3xl font-bold mb-2">
              <span className="gradient-text">Verify Security</span>
            </h2>
            <p className="text-[var(--text-secondary)] text-sm">
              Don't trust us — verify the cryptographic proof yourself.
            </p>
          </motion.div>

          {attestationLoading ? (
            <VerificationPanelSkeleton />
          ) : (
            <VerificationPanel />
          )}
        </div>
      </section>

      {/* Footer */}
      <footer className="py-8 border-t border-white/5">
        <div className="w-full max-w-4xl mx-auto px-8 md:px-12">
          <p className="text-[var(--text-muted)] text-xs text-center">
            Powered by{' '}
            <a href="https://near.ai" target="_blank" rel="noopener noreferrer" className="hover:text-[var(--text-secondary)] transition-colors">
              NEAR AI
            </a>
            {' · '}
            Secured by{' '}
            <a href="https://phala.network" target="_blank" rel="noopener noreferrer" className="hover:text-[var(--text-secondary)] transition-colors">
              Phala Network
            </a>
            {' · '}
            <a href="https://github.com/zmanian/signal-bot-tee" target="_blank" rel="noopener noreferrer" className="hover:text-[var(--text-secondary)] transition-colors">
              Open Source
            </a>
          </p>
        </div>
      </footer>
    </div>
  )
}

export default App
