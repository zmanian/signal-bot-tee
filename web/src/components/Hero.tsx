import { motion } from 'framer-motion'
import { Shield, Lock, Cpu } from 'lucide-react'

export function Hero() {
  return (
    <section className="section min-h-[90vh] flex items-center justify-center">
      <div className="container text-center">
        {/* Animated badge */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.6 }}
          className="inline-flex items-center gap-2 px-4 py-2 mb-8 rounded-full glass-card text-sm font-medium"
        >
          <span className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />
          <span className="text-[var(--text-secondary)]">Secured by Intel TDX</span>
        </motion.div>

        {/* Main title */}
        <motion.h1
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.7, delay: 0.1 }}
          className="text-5xl md:text-7xl font-bold mb-6 leading-tight"
        >
          <span className="gradient-text">Private AI</span>
          <br />
          <span className="text-[var(--text-primary)]">Conversations</span>
        </motion.h1>

        {/* Subtitle */}
        <motion.p
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.7, delay: 0.2 }}
          className="text-xl md:text-2xl text-[var(--text-secondary)] mb-12 max-w-2xl mx-auto leading-relaxed"
        >
          Chat with AI through Signal, protected by hardware security.
          <br className="hidden md:block" />
          <span className="text-[var(--text-muted)]">Verify everything. Trust cryptography.</span>
        </motion.p>

        {/* Trust cards */}
        <motion.div
          initial={{ opacity: 0, y: 40 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.3 }}
          className="flex flex-col md:flex-row gap-6 justify-center items-center mb-16"
        >
          <TrustCard
            icon={<Shield className="w-5 h-5" />}
            title="End-to-End Encrypted"
            description="Signal protocol protects every message"
            delay={0.4}
          />
          <TrustCard
            icon={<Cpu className="w-5 h-5" />}
            title="TEE Protected"
            description="Runs in Intel TDX secure enclave"
            delay={0.5}
          />
          <TrustCard
            icon={<Lock className="w-5 h-5" />}
            title="Verifiable"
            description="Cryptographic proof you can check"
            delay={0.6}
          />
        </motion.div>

        {/* Scroll indicator */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 1, delay: 1 }}
          className="mt-16"
        >
          <motion.div
            animate={{ y: [0, 8, 0] }}
            transition={{ duration: 2, repeat: Infinity, ease: "easeInOut" }}
            className="w-6 h-10 mx-auto rounded-full border-2 border-[var(--text-muted)] flex items-start justify-center p-2"
          >
            <div className="w-1.5 h-3 bg-[var(--text-muted)] rounded-full" />
          </motion.div>
        </motion.div>
      </div>
    </section>
  )
}

function TrustCard({
  icon,
  title,
  description,
  delay,
}: {
  icon: React.ReactNode
  title: string
  description: string
  delay: number
}) {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.9 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.5, delay }}
      whileHover={{ scale: 1.02 }}
      className="glass-card px-8 py-6 flex items-center gap-5 min-w-[320px]"
    >
      <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-[var(--accent-start)] to-[var(--accent-end)] flex items-center justify-center text-white">
        {icon}
      </div>
      <div className="text-left">
        <h3 className="font-semibold text-[var(--text-primary)]">{title}</h3>
        <p className="text-sm text-[var(--text-muted)]">{description}</p>
      </div>
    </motion.div>
  )
}
