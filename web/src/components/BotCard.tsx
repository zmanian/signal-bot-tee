import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { MessageCircle, Shield, ChevronDown, Copy, Check, Cpu } from 'lucide-react'
import type { Bot } from '../lib/api'

interface BotCardProps {
  bot: Bot
  index: number
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
      className="glass-card p-6 md:p-8"
    >
      {/* Header */}
      <div className="flex items-start justify-between mb-4">
        <div className="flex items-center gap-4">
          <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-[var(--accent-start)] to-[var(--accent-end)] flex items-center justify-center text-white text-xl font-bold shadow-lg">
            {bot.username.charAt(0).toUpperCase()}
          </div>
          <div>
            <h3 className="text-xl font-semibold text-[var(--text-primary)]">@{bot.username}</h3>
            <p className="text-sm text-[var(--text-muted)]">Signal Bot</p>
          </div>
        </div>
        <div className="flex items-center gap-2 text-emerald-400 text-sm font-medium bg-emerald-500/10 px-3 py-1.5 rounded-full">
          <span className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />
          Online
        </div>
      </div>

      {/* Description and Model */}
      {(bot.description || bot.model) && (
        <div className="mb-6 space-y-3">
          {bot.description && (
            <p className="text-sm text-[var(--text-secondary)] leading-relaxed">
              {bot.description}
            </p>
          )}
          {bot.model && (
            <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
              <Cpu className="w-3.5 h-3.5" />
              <span className="font-mono bg-white/5 px-2 py-1 rounded-md">
                {bot.model}
              </span>
            </div>
          )}
        </div>
      )}

      {/* Spacer when no description/model */}
      {!bot.description && !bot.model && <div className="mb-2" />}

      {/* Actions */}
      <div className="flex flex-col sm:flex-row gap-3 mb-4">
        <a
          href={bot.signal_link}
          target="_blank"
          rel="noopener noreferrer"
          className="glass-button glass-button-primary flex items-center justify-center gap-2 flex-1"
        >
          <MessageCircle className="w-5 h-5" />
          Message on Signal
        </a>
        <button
          onClick={() => setExpanded(!expanded)}
          className="glass-button flex items-center justify-center gap-2"
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
            <div className="pt-4 border-t border-white/5">
              <div className="flex items-center justify-between mb-3">
                <h4 className="font-medium text-[var(--text-primary)]">Signal Identity Key</h4>
                {bot.identity_key && (
                  <button
                    onClick={copyIdentityKey}
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

              {bot.identity_key ? (
                <div className="code-block text-xs break-all">
                  {bot.identity_key}
                </div>
              ) : (
                <p className="text-sm text-[var(--text-muted)] italic">
                  Identity key not available
                </p>
              )}

              <p className="text-xs text-[var(--text-muted)] mt-3 leading-relaxed">
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
    <div className="glass-card p-6 md:p-8">
      <div className="flex items-start justify-between mb-6">
        <div className="flex items-center gap-4">
          <div className="w-14 h-14 rounded-2xl skeleton" />
          <div>
            <div className="h-6 w-32 skeleton mb-2" />
            <div className="h-4 w-20 skeleton" />
          </div>
        </div>
        <div className="h-8 w-20 skeleton rounded-full" />
      </div>
      <div className="flex gap-3">
        <div className="h-12 flex-1 skeleton rounded-2xl" />
        <div className="h-12 flex-1 skeleton rounded-2xl" />
      </div>
    </div>
  )
}
