import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from '@codemirror/language'
import type { StreamParser } from '@codemirror/language'
import { tags } from '@lezer/highlight'
import type { Extension } from '@codemirror/state'

/**
 * Syntax support for the expression language designs are written in.
 *
 * A stream parser rather than a full grammar. Expressions here are short — a
 * line or two binding a quantity — and the thing an author needs from the editor
 * is to see at a glance which names are being called and which are their own.
 * A hand-written tokeniser gives that, and a Lezer grammar would have to be kept
 * in step with a language defined in Rust for no visible gain.
 */

const KEYWORDS = new Set(['if', 'then', 'else', 'import', 'as', 'export', 'true', 'false'])

const parser: StreamParser<{ inComment: boolean }> = {
  name: 'squiggle',
  startState: () => ({ inComment: false }),
  token(stream, state) {
    if (state.inComment) {
      if (stream.match(/.*?\*\//)) state.inComment = false
      else stream.skipToEnd()
      return 'comment'
    }
    if (stream.eatSpace()) return null

    if (stream.match('/*')) {
      state.inComment = true
      return 'comment'
    }
    if (stream.match('//')) {
      stream.skipToEnd()
      return 'comment'
    }

    // A unit annotation runs to the end of its clause and is neither a name nor
    // a number, so it gets its own token: it is documentation the checker reads.
    if (stream.match('::')) {
      stream.match(/[^=\n]*/)
      return 'meta'
    }

    if (stream.match(/^"(?:[^"\\]|\\.)*"?/)) return 'string'
    if (stream.match(/^\d+(\.\d+)?([eE][+-]?\d+)?[a-zA-Z%]*/)) return 'number'

    // Dotted names are one token. Splitting `Little.occupancy` into three would
    // highlight the namespace as though it were a variable in scope.
    if (stream.match(/^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*/)) {
      const word = stream.current()
      if (KEYWORDS.has(word)) return 'keyword'
      if (word.includes('.')) return 'function'
      // A name immediately followed by an opening bracket is being called.
      return stream.peek() === '(' ? 'function' : 'variableName'
    }

    if (stream.match(/^(->|[+\-*/^<>=!]=?|\|\||&&)/)) return 'operator'
    stream.next()
    return null
  },
}

export const squiggleLanguage = StreamLanguage.define(parser)

/**
 * Colours chosen to separate the three things an author confuses.
 *
 * What the language provides, what this design provides, and what is being
 * written down here are different kinds of name, and the mistake worth catching
 * early is referring to something that does not exist in any of them.
 */
const highlight = HighlightStyle.define([
  { tag: tags.comment, color: '#69716d', fontStyle: 'italic' },
  { tag: tags.keyword, color: '#9a3e31', fontWeight: '650' },
  { tag: tags.number, color: '#245746' },
  { tag: tags.string, color: '#245746' },
  { tag: tags.function(tags.variableName), color: '#2a5b8f' },
  { tag: tags.variableName, color: '#25292b' },
  { tag: tags.operator, color: '#69716d' },
  { tag: tags.meta, color: '#6f4f05', fontStyle: 'italic' },
])

/** What an expression may refer to, beyond the language itself. */
export interface ExpressionScope {
  /** Names the language provides. */
  builtins: string[]
  /** Shared quantities this design declares. */
  quantities: { name: string; unit: string; summary: string }[]
  /** Names the surrounding component or mutator binds. */
  locals: { name: string; detail: string }[]
}

function options(scope: ExpressionScope): Completion[] {
  return [
    // Ordered so that the design's own vocabulary is offered before the
    // language's. An author reaching for a name almost always wants one of
    // their own, and there are far more builtins to scroll past.
    ...scope.locals.map((local) => ({
      label: local.name,
      type: 'property',
      detail: local.detail,
      boost: 2,
    })),
    ...scope.quantities.map((quantity) => ({
      label: quantity.name,
      type: 'variable',
      detail: quantity.unit,
      info: quantity.summary,
      boost: 1,
    })),
    ...scope.builtins.map((name) => ({
      label: name,
      type: 'function',
      apply: `${name}(`,
    })),
  ]
}

/**
 * Completion over the names in scope.
 *
 * The match has to be written out rather than left to CodeMirror's default word
 * boundary, which stops at a dot: typing `Little.` would otherwise be read as
 * the start of a fresh word and offer the whole vocabulary. Filtering the
 * options against what has been typed is left to CodeMirror, which is what
 * `validFor` tells it to keep doing as the word grows.
 */
export function squiggleCompletion(scope: () => ExpressionScope): Extension {
  return autocompletion({
    override: [
      (context: CompletionContext): CompletionResult | null => {
        const word = context.matchBefore(/[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z0-9_]*)?/)
        if (!word || (word.from === word.to && !context.explicit)) return null
        return {
          from: word.from,
          options: options(scope()),
          validFor: /^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z0-9_]*)?$/,
        }
      },
    ],
  })
}

export function squiggleSupport(scope: () => ExpressionScope): Extension[] {
  return [squiggleLanguage, syntaxHighlighting(highlight), squiggleCompletion(scope)]
}
