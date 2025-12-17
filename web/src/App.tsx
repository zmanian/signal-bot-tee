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
    <div className="min-h-screen w-full flex flex-col">
      {/* Floating particles */}
      <div className="particle" />
      <div className="particle" />
      <div className="particle" />

      {/* Hero Section */}
      <section className="w-full py-20 md:py-28">
        <div className="max-w-6xl mx-auto px-6 md:px-12 lg:px-20">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
            className="flex flex-col items-center text-center"
          >
            <div className="inline-flex items-center gap-2 px-4 py-2 mb-8 rounded-full glass-card text-sm font-medium">
              <span className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />
              <span className="text-[var(--text-secondary)]">Secured by Intel TDX</span>
            </div>

            <h1 className="text-5xl md:text-7xl font-bold mb-6 leading-tight">
              <span className="gradient-text">Private AI</span>
              <span className="text-[var(--text-primary)]"> Conversations</span>
            </h1>

            <p className="text-xl md:text-2xl text-[var(--text-secondary)] mb-10 max-w-2xl">
              Chat with AI through Signal, protected by hardware security.
            </p>

            <div className="flex flex-wrap justify-center gap-8 text-sm text-[var(--text-muted)]">
              <div className="flex items-center gap-2">
                <Shield className="w-5 h-5 text-[var(--accent-start)]" />
                <span>End-to-End Encrypted</span>
              </div>
              <div className="flex items-center gap-2">
                <Cpu className="w-5 h-5 text-[var(--accent-mid)]" />
                <span>TEE Protected</span>
              </div>
              <div className="flex items-center gap-2">
                <Lock className="w-5 h-5 text-[var(--accent-end)]" />
                <span>Verifiable</span>
              </div>
            </div>
          </motion.div>
        </div>
      </section>

      {/* Bot Cards Section */}
      <section className="w-full pb-20 md:pb-28">
        <div className="max-w-2xl mx-auto px-6 md:px-12 lg:px-20">
          {botsError ? (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-10 flex flex-col items-center text-center"
            >
              <div className="w-16 h-16 mb-4 rounded-full bg-amber-500/10 flex items-center justify-center">
                <AlertCircle className="w-8 h-8 text-amber-400" />
              </div>
              <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-3">
                Could not load bots
              </h3>
              <p className="text-[var(--text-muted)] mb-6">
                The bot directory is temporarily unavailable.
              </p>
              <button
                onClick={() => refetchBots()}
                className="glass-button flex items-center gap-2"
              >
                <RefreshCw className="w-4 h-4" />
                Retry
              </button>
            </motion.div>
          ) : botsLoading ? (
            <BotCardSkeleton />
          ) : bots && bots.length > 0 ? (
            <div className="space-y-8">
              {bots.map((bot, index) => (
                <BotCard key={bot.username} bot={bot} index={index} featured={bots.length === 1} />
              ))}
            </div>
          ) : (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-8 flex flex-col items-center text-center"
            >
              <p className="text-[var(--text-muted)]">No bots registered yet.</p>
            </motion.div>
          )}
        </div>
      </section>

      {/* Verification Section */}
      <section id="verify" className="w-full py-20 md:py-28 border-t border-white/5">
        <div className="max-w-3xl mx-auto px-6 md:px-12 lg:px-20">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
            className="flex flex-col items-center text-center mb-12"
          >
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              <span className="gradient-text">Verify Security</span>
            </h2>
            <p className="text-[var(--text-secondary)] text-lg">
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
      <footer className="w-full py-10 border-t border-white/5 mt-auto">
        <div className="max-w-6xl mx-auto px-6 md:px-12 lg:px-20">
          <p className="text-[var(--text-muted)] text-sm text-center">
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
