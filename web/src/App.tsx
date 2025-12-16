import { Hero } from './components/Hero'
import { BotCard, BotCardSkeleton } from './components/BotCard'
import { VerificationPanel, VerificationPanelSkeleton } from './components/VerificationPanel'
import { useBots } from './hooks/useBots'
import { useAttestation } from './hooks/useAttestation'
import { motion } from 'framer-motion'
import { AlertCircle, RefreshCw } from 'lucide-react'

function App() {
  const { data: bots, isLoading: botsLoading, error: botsError, refetch: refetchBots } = useBots()
  const { isLoading: attestationLoading } = useAttestation()

  return (
    <>
      {/* Floating particles */}
      <div className="particle" />
      <div className="particle" />
      <div className="particle" />
      <div className="particle" />
      <div className="particle" />

      {/* Hero section */}
      <Hero />

      {/* Bots section */}
      <section id="bots" className="section">
        <div className="container">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.6 }}
            className="text-center mb-12"
          >
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              <span className="gradient-text">Available Bots</span>
            </h2>
            <p className="text-[var(--text-secondary)] max-w-2xl mx-auto">
              Select a bot to start a private conversation. Each bot runs in its own secure enclave.
            </p>
          </motion.div>

          {botsError ? (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-8 text-center max-w-md mx-auto"
            >
              <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-amber-500/10 flex items-center justify-center">
                <AlertCircle className="w-8 h-8 text-amber-400" />
              </div>
              <h3 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
                Could not load bots
              </h3>
              <p className="text-[var(--text-muted)] mb-6">
                The bot directory is temporarily unavailable.
              </p>
              <button
                onClick={() => refetchBots()}
                className="glass-button flex items-center gap-2 mx-auto"
              >
                <RefreshCw className="w-4 h-4" />
                Retry
              </button>
            </motion.div>
          ) : botsLoading ? (
            <div className="grid md:grid-cols-2 gap-6 max-w-4xl mx-auto">
              <BotCardSkeleton />
              <BotCardSkeleton />
            </div>
          ) : bots && bots.length > 0 ? (
            <div className="grid md:grid-cols-2 gap-6 max-w-4xl mx-auto">
              {bots.map((bot, index) => (
                <BotCard key={bot.username} bot={bot} index={index} />
              ))}
            </div>
          ) : (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-card p-8 text-center max-w-md mx-auto"
            >
              <p className="text-[var(--text-muted)]">No bots registered yet.</p>
            </motion.div>
          )}
        </div>
      </section>

      {/* Verification section */}
      <section id="verify" className="section">
        <div className="container max-w-2xl">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.6 }}
            className="text-center mb-12"
          >
            <h2 className="text-3xl md:text-4xl font-bold mb-4">
              <span className="gradient-text">Verify Security</span>
            </h2>
            <p className="text-[var(--text-secondary)] max-w-2xl mx-auto">
              Don't trust us — verify the cryptographic proof that your messages are protected.
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
      <footer className="py-12 text-center">
        <div className="container">
          <p className="text-[var(--text-muted)] text-sm">
            Powered by{' '}
            <a
              href="https://near.ai"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[var(--accent-start)] hover:text-[var(--accent-end)] transition-colors"
            >
              NEAR AI
            </a>
            {' · '}
            Secured by{' '}
            <a
              href="https://phala.network"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[var(--accent-start)] hover:text-[var(--accent-end)] transition-colors"
            >
              Phala Network
            </a>
            {' · '}
            <a
              href="https://github.com/zmanian/signal-bot-tee"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[var(--accent-start)] hover:text-[var(--accent-end)] transition-colors"
            >
              Open Source
            </a>
          </p>
        </div>
      </footer>
    </>
  )
}

export default App
