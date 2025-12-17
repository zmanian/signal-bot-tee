import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { MessageCircle, Shield, ChevronDown, Copy, Check, Cpu } from 'lucide-react'
import type { Bot } from '../lib/api'

interface BotCardProps {
  bot: Bot
  index: number
  featured?: boolean
}

export function BotCard({ bot, index }: BotCardProps) {
  const [expanded, setExpanded] = useState(false)
  const [copied, setCopied] = useState(false)

  const copyIdentityKey = async () => {
    if (bot.identity_key) {
      await navigator.clipboard.writeText(bot.identity_key)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 30 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.5, delay: index * 0.1 }}
      className="glass-card p-8 md:p-12"
    >
      {/* Avatar and Name - Centered */}
      <div className="flex flex-col items-center text-center mb-8">
        <div className="w-24 h-24 rounded-2xl bg-gradient-to-br from-[var(--accent-start)] to-[var(--accent-end)] flex items-center justify-center text-white text-4xl font-bold shadow-lg mb-5">
          {bot.username.charAt(0).toUpperCase()}
        </div>
        <h3 className="text-2xl md:text-3xl font-semibold text-[var(--text-primary)] mb-1">@{bot.username}</h3>
        <p className="text-base text-[var(--text-muted)] mb-3">Signal Bot</p>
        <div className="inline-flex items-center gap-2 text-emerald-400 text-sm font-medium bg-emerald-500/10 px-4 py-2 rounded-full">
          <span className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />
          Online
        </div>
      </div>

      {/* Description and Model - Centered */}
      {(bot.description || bot.model) && (
        <div className="flex flex-col items-center text-center mb-8 space-y-4">
          {bot.description && (
            <p className="text-base md:text-lg text-[var(--text-secondary)] leading-relaxed max-w-lg">
              {bot.description}
            </p>
          )}
          {bot.model && (
            <div className="inline-flex items-center gap-2 text-sm text-[var(--text-muted)]">
              <Cpu className="w-4 h-4" />
              <span className="font-mono bg-white/5 px-3 py-1.5 rounded-lg">
                {bot.model}
              </span>
            </div>
          )}
        </div>
      )}

      {/* Actions - Centered */}
      <div className="flex flex-col sm:flex-row gap-4 justify-center max-w-lg mx-auto">
        <a
          href={bot.signal_link}
          target="_blank"
          rel="noopener noreferrer"
          className="glass-button glass-button-primary flex items-center justify-center gap-3 flex-1 py-4 text-base"
        >
          <MessageCircle className="w-5 h-5" />
          Message on Signal
        </a>
        <button
          onClick={() => setExpanded(!expanded)}
          className="glass-button flex items-center justify-center gap-3 flex-1 py-4 text-base"
        >
          <Shield className="w-5 h-5" />
          Verify Security
          <motion.div
            animate={{ rotate: expanded ? 180 : 0 }}
            transition={{ duration: 0.3 }}
          >
            <ChevronDown className="w-4 h-4" />
          </motion.div>
        </button>
      </div>

      {/* Expandable identity key section */}
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.3 }}
            className="overflow-hidden"
          >
            <div className="pt-8 mt-8 border-t border-white/10 max-w-lg mx-auto">
              <div className="flex items-center justify-between mb-4">
                <h4 className="font-medium text-[var(--text-primary)]">Signal Identity Key</h4>
                {bot.identity_key && (
                  <button
                    onClick={copyIdentityKey}
                    className="text-sm text-[var(--accent-start)] hover:text-[var(--accent-end)] flex items-center gap-2 transition-colors"
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

              {bot.identity_key ? (
                <div className="code-block text-xs break-all">
                  {bot.identity_key}
                </div>
              ) : (
                <p className="text-sm text-[var(--text-muted)] italic">
                  Identity key not available
                </p>
              )}

              <p className="text-xs text-[var(--text-muted)] mt-4 leading-relaxed">
                Compare this with Signal app: Open chat → Tap contact name → View Safety Number
              </p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  )
}

export function BotCardSkeleton() {
  return (
    <div className="glass-card p-8 md:p-12">
      <div className="flex flex-col items-center mb-8">
        <div className="w-24 h-24 rounded-2xl skeleton mb-5" />
        <div className="h-8 w-48 skeleton mb-2" />
        <div className="h-5 w-24 skeleton mb-3" />
        <div className="h-8 w-20 skeleton rounded-full" />
      </div>
      <div className="flex gap-4 justify-center max-w-lg mx-auto">
        <div className="h-14 flex-1 skeleton rounded-xl" />
        <div className="h-14 flex-1 skeleton rounded-xl" />
      </div>
    </div>
  )
}
