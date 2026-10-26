/**
 * Conversation Virtualizer Hook
 *
 * Shared TanStack Virtual configuration for the conversation list.
 * Owns the virtualizer instance, measurement, and imperative scroll helpers.
 */

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type RefObject,
} from 'react';
import {
  useVirtualizer,
  measureElement as defaultMeasureElement,
} from '@tanstack/react-virtual';
import type { Virtualizer, VirtualItem } from '@tanstack/react-virtual';

import {
  type ConversationRow,
  SIZE_ESTIMATE_PX,
  estimateSizeForRow,
  findPreviousUserMessageIndex,
} from './conversation-row-model';
import {
  NEAR_BOTTOM_THRESHOLD_PX,
  isNearBottom,
} from './conversation-scroll-commands';

// TanStack Virtual's ScrollBehavior ('auto' | 'smooth' | 'instant') shadows
// the DOM ScrollBehavior. Use a narrow type to avoid TS2322 mismatches.
type ScrollToOptionsBehavior = 'auto' | 'smooth';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Number of items to render beyond the visible area in each direction. */
const OVERSCAN = 8;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ConversationVirtualizerOptions {
  /** The semantic row model driving the list (virtualized head only). */
  rows: ConversationRow[];

  /**
   * Total number of conversation rows (virtualized + unvirtualized tail).
   * The bottom-lock correction must fire when ANY row is added — including
   * unvirtualized tail rows that don't change `rows.length` or `totalSize`.
   * Without this, streaming entries appended to the tail silently grow the
   * scroll container while the correction never fires.
   */
  totalRowCount: number;

  /** Ref to the scrollable container element. */
  scrollContainerRef: RefObject<HTMLDivElement | null>;

  /**
   * Called when the at-bottom state changes. Shells use this to show/hide
   * the scroll-to-bottom affordance.
   */
  onAtBottomChange?: (atBottom: boolean) => void;
  /** User intent, independent of transient measured geometry. */
  onFollowChange?: (following: boolean) => void;

  shouldSuppressSizeAdjustment?: () => boolean;
  contentRef?: RefObject<HTMLDivElement | null>;
  /** Preserve an existing reading anchor when tail DOM resizes without React. */
  onContentResize?: () => void;
  /** Capture the viewport after an explicit user scrolling gesture. */
  onReadingPositionChange?: () => void;
}

export interface ConversationVirtualizerResult {
  /** The TanStack Virtual virtualizer instance. */
  virtualizer: Virtualizer<HTMLDivElement, Element>;

  /** Virtual items currently in the render window (including overscan). */
  virtualItems: VirtualItem[];

  /** Total pixel size of all items (for the scroll spacer). */
  totalSize: number;

  /**
   * Ref callback for row DOM elements. Attach to each rendered row's
   * container element alongside `data-index={virtualItem.index}`.
   * TanStack Virtual uses this to measure real DOM heights and attach
   * a ResizeObserver for automatic re-measurement on size changes.
   */
  measureElement: (node: Element | null) => void;

  /** Scroll to the absolute bottom of the list. */
  scrollToBottom: (behavior?: ScrollToOptionsBehavior) => void;

  /** Scroll to a specific row index. */
  scrollToIndex: (
    index: number,
    options?: {
      align?: 'start' | 'center' | 'end';
      behavior?: ScrollToOptionsBehavior;
    }
  ) => void;

  /**
   * Scroll to the previous user message relative to the first visible item.
   * Returns true if a target was found and scrolled to, false otherwise.
   */
  scrollToPreviousUserMessage: () => boolean;

  /**
   * Whether the scroll container is currently near the bottom.
   * Reactive — updates via scroll event listener, not just point-in-time.
   */
  isAtBottom: boolean;

  /** Follow intent for data updates; geometry alone must not resume reading. */
  checkIsAtBottom: () => boolean;

  /**
   * Release the bottom-lock. Call when navigating away from the
   * bottom (e.g., scrollToPreviousUserMessage).
   */
  releaseBottomLock: () => void;

  /**
   * Look up the ConversationRow index for a given virtual item.
   * Since our virtualizer uses identity mapping (no lane reordering),
   * this is simply `virtualItem.index`.
   */
  rowIndexForVirtualItem: (item: VirtualItem) => number;

  /**
   * Look up the ConversationRow for a given virtual item.
   * Returns undefined if the index is out of bounds.
   */
  rowForVirtualItem: (item: VirtualItem) => ConversationRow | undefined;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Configure and return a TanStack Virtual virtualizer for the conversation list.
 *
 * This hook is the single source of virtualizer configuration. It is consumed
 * by `ConversationListContainer` and must not be duplicated across shells.
 */
export function useConversationVirtualizer({
  rows,
  totalRowCount,
  scrollContainerRef,
  onAtBottomChange,
  onFollowChange,
  shouldSuppressSizeAdjustment,
  contentRef,
  onContentResize,
  onReadingPositionChange,
}: ConversationVirtualizerOptions): ConversationVirtualizerResult {
  const readingPositionCallbackRef = useRef(onReadingPositionChange);
  readingPositionCallbackRef.current = onReadingPositionChange;
  const followingRef = useRef(true);
  const onFollowChangeRef = useRef(onFollowChange);
  onFollowChangeRef.current = onFollowChange;
  const setFollowing = useCallback((following: boolean) => {
    if (followingRef.current === following) return;
    followingRef.current = following;
    onFollowChangeRef.current?.(following);
  }, []);

  // -------------------------------------------------------------------------
  // Virtualizer instance
  // -------------------------------------------------------------------------

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: (index) => {
      const row = rows[index];
      if (!row) return SIZE_ESTIMATE_PX.medium;
      const containerWidth = scrollContainerRef.current?.clientWidth ?? null;
      return estimateSizeForRow(row, containerWidth);
    },
    getItemKey: (index) => {
      const row = rows[index];
      return row ? row.semanticKey : index;
    },
    overscan: OVERSCAN,
    measureElement: defaultMeasureElement,
    useAnimationFrameWithResizeObserver: false,
  });

  // -------------------------------------------------------------------------
  // shouldAdjustScrollPositionOnItemSizeChange
  //
  // Preserve the reader's position only when a row fully above the viewport
  // changes size. Mid-list flicker happens when we compensate for rows that
  // are still visible or below the viewport, because those corrections can
  // move the render window and trigger another measurement pass.
  // -------------------------------------------------------------------------

  useEffect(() => {
    virtualizer.shouldAdjustScrollPositionOnItemSizeChange = (
      item,
      _delta,
      instance
    ) => {
      const scrollElement = scrollContainerRef.current;
      const viewportHeight =
        scrollElement?.clientHeight ?? instance.scrollRect?.height ?? 0;
      const scrollOffset =
        scrollElement?.scrollTop ?? instance.scrollOffset ?? 0;
      const totalScrollableSize =
        scrollElement?.scrollHeight ?? instance.getTotalSize();
      const remainingDistance =
        totalScrollableSize - (scrollOffset + viewportHeight);
      const isItemFullyAboveViewport = item.end <= scrollOffset;
      const isBottomLocked = followingRef.current;

      const shouldAdjust =
        !isBottomLocked &&
        !shouldSuppressSizeAdjustment?.() &&
        isItemFullyAboveViewport &&
        remainingDistance > NEAR_BOTTOM_THRESHOLD_PX;

      return shouldAdjust;
    };

    return () => {
      virtualizer.shouldAdjustScrollPositionOnItemSizeChange = undefined;
    };
  }, [scrollContainerRef, shouldSuppressSizeAdjustment, virtualizer]);

  // -------------------------------------------------------------------------
  // Reactive isAtBottom state
  // -------------------------------------------------------------------------

  const [isAtBottomState, setIsAtBottomState] = useState(true);
  const onAtBottomChangeRef = useRef(onAtBottomChange);
  onAtBottomChangeRef.current = onAtBottomChange;
  const lastAtBottomRef = useRef(true);

  const syncIsAtBottom = useCallback(() => {
    const el = scrollContainerRef.current;
    const nextValue = el
      ? isNearBottom(el.scrollTop, el.clientHeight, el.scrollHeight)
      : true;

    if (nextValue !== lastAtBottomRef.current) {
      lastAtBottomRef.current = nextValue;
      setIsAtBottomState(nextValue);
      onAtBottomChangeRef.current?.(nextValue);
      return;
    }

    setIsAtBottomState((current) =>
      current === nextValue ? current : nextValue
    );
  }, [scrollContainerRef]);

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    let userScrollPending = false;
    let gestureEndTimer: ReturnType<typeof setTimeout> | undefined;
    const markUserScroll = () => {
      userScrollPending = true;
      clearTimeout(gestureEndTimer);
      // Retain the gesture through touch momentum / smooth keyboard scrolling.
      gestureEndTimer = setTimeout(() => {
        userScrollPending = false;
      }, 180);
    };
    let towardBottom = false;
    let scrollbarDrag = false;
    let previousTop = el.scrollTop;
    let touchY: number | null = null;
    const handleScroll = () => {
      // Scroll events also come from virtual measurements, browser clamping,
      // and anchor restoration. They cannot by themselves revoke user intent.
      if (scrollbarDrag && el.scrollTop < previousTop) setFollowing(false);
      if (towardBottom && el.scrollHeight - el.clientHeight - el.scrollTop <= 2)
        setFollowing(true);
      if (!followingRef.current && (userScrollPending || scrollbarDrag)) {
        readingPositionCallbackRef.current?.();
        markUserScroll();
      }
      previousTop = el.scrollTop;
      syncIsAtBottom();
    };
    const up = () => {
      markUserScroll();
      towardBottom = false;
      setFollowing(false);
      readingPositionCallbackRef.current?.();
    };
    const handleWheel = (event: WheelEvent) => {
      if (event.deltaY < 0) up();
      else if (event.deltaY > 0) {
        markUserScroll();
        towardBottom = true;
      }
    };
    const handleTouchStart = (event: TouchEvent) => {
      touchY = event.touches[0]?.clientY ?? null;
    };
    const handleTouchMove = (event: TouchEvent) => {
      const next = event.touches[0]?.clientY ?? null;
      if (touchY !== null && next !== null) {
        if (next > touchY) up();
        else if (next < touchY) {
          markUserScroll();
          towardBottom = true;
        }
      }
      touchY = next;
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        target instanceof Element &&
        target.closest('input,textarea,[contenteditable="true"]')
      )
        return;
      if (
        ['ArrowUp', 'PageUp', 'Home'].includes(event.key) ||
        (event.key === ' ' && event.shiftKey)
      )
        up();
      else if (['ArrowDown', 'PageDown', 'End', ' '].includes(event.key)) {
        markUserScroll();
        towardBottom = true;
      }
    };
    const handlePointerDown = (event: PointerEvent) => {
      // Native scrollbar events target the scroll container, not its rows.
      scrollbarDrag = event.target === el;
      previousTop = el.scrollTop;
    };
    const handlePointerUp = () => {
      scrollbarDrag = false;
    };
    el.addEventListener('wheel', handleWheel, { passive: true });
    el.addEventListener('touchstart', handleTouchStart, { passive: true });
    el.addEventListener('touchmove', handleTouchMove, { passive: true });
    el.addEventListener('keydown', handleKeyDown);
    el.addEventListener('pointerdown', handlePointerDown);
    window.addEventListener('pointerup', handlePointerUp);
    window.addEventListener('pointercancel', handlePointerUp);
    el.addEventListener('scroll', handleScroll, { passive: true });
    handleScroll();
    return () => {
      clearTimeout(gestureEndTimer);
      el.removeEventListener('wheel', handleWheel);
      el.removeEventListener('touchstart', handleTouchStart);
      el.removeEventListener('touchmove', handleTouchMove);
      el.removeEventListener('keydown', handleKeyDown);
      el.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('pointerup', handlePointerUp);
      window.removeEventListener('pointercancel', handlePointerUp);
      el.removeEventListener('scroll', handleScroll);
    };
  }, [scrollContainerRef, setFollowing, syncIsAtBottom]);

  // -------------------------------------------------------------------------
  // Derived state
  // -------------------------------------------------------------------------

  const virtualItems = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();

  useLayoutEffect(() => {
    if (!followingRef.current) {
      syncIsAtBottom();
      return;
    }

    const el = scrollContainerRef.current;
    if (!el) return;

    const maxScroll = el.scrollHeight - el.clientHeight;
    if (maxScroll > 0 && Math.abs(maxScroll - el.scrollTop) > 1) {
      el.scrollTop = maxScroll;
    }
    syncIsAtBottom();
  }, [rows, totalRowCount, totalSize, syncIsAtBottom, scrollContainerRef]);

  // The tail is outside TanStack's measurements. Images, markdown and streamed
  // text can grow without changing row count or virtual head size.
  useEffect(() => {
    const content = contentRef?.current;
    const el = scrollContainerRef.current;
    if (!content || !el) return;
    let frame: number | null = null;
    const correct = () => {
      if (followingRef.current) {
        el.scrollTop = Math.max(0, el.scrollHeight - el.clientHeight);
      } else onContentResize?.();
      syncIsAtBottom();
    };
    const observer = new ResizeObserver(() => {
      correct();
      // Virtual head measurements can commit after this observer delivery.
      if (frame !== null) cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        frame = null;
        correct();
      });
    });
    observer.observe(content);
    observer.observe(el);
    return () => {
      observer.disconnect();
      if (frame !== null) cancelAnimationFrame(frame);
    };
  }, [contentRef, scrollContainerRef, syncIsAtBottom, onContentResize]);

  // -------------------------------------------------------------------------
  // Imperative helpers
  // -------------------------------------------------------------------------

  const scrollToBottom = useCallback(
    (_behavior: ScrollToOptionsBehavior = 'auto') => {
      const el = scrollContainerRef.current;
      if (!el) return;
      // A jump establishes intent synchronously, before the send's rAF update.
      // Native smooth targets become stale under streaming/virtual resizing.
      setFollowing(true);
      el.scrollTop = Math.max(0, el.scrollHeight - el.clientHeight);
      syncIsAtBottom();
    },
    [scrollContainerRef, setFollowing, syncIsAtBottom]
  );

  const scrollToIndex = useCallback(
    (
      index: number,
      options?: {
        align?: 'start' | 'center' | 'end';
        behavior?: ScrollToOptionsBehavior;
      }
    ) => {
      setFollowing(false);
      virtualizer.scrollToIndex(index, {
        align: options?.align ?? 'start',
        behavior: options?.behavior ?? 'smooth',
      });
    },
    [setFollowing, virtualizer]
  );

  const scrollToPreviousUserMessage = useCallback((): boolean => {
    const scrollEl = scrollContainerRef.current;
    const items = virtualizer.getVirtualItems();
    if (items.length === 0 || rows.length === 0 || !scrollEl) return false;

    const firstVisibleIndex =
      virtualizer.getVirtualItemForOffset(scrollEl.scrollTop)?.index ??
      items[0].index;
    const targetIndex = findPreviousUserMessageIndex(rows, firstVisibleIndex);

    if (targetIndex < 0) return false;

    setFollowing(false);
    virtualizer.scrollToIndex(targetIndex, {
      align: 'start',
      behavior: 'smooth',
    });
    return true;
  }, [scrollContainerRef, setFollowing, virtualizer, rows]);

  const checkIsAtBottom = useCallback((): boolean => followingRef.current, []);

  const releaseBottomLock = useCallback(() => {
    setFollowing(false);
  }, [setFollowing]);

  // -------------------------------------------------------------------------
  // Row ↔ VirtualItem mapping
  // -------------------------------------------------------------------------

  const rowIndexForVirtualItem = useCallback(
    (item: VirtualItem): number => item.index,
    []
  );

  const rowForVirtualItem = useCallback(
    (item: VirtualItem): ConversationRow | undefined => rows[item.index],
    [rows]
  );

  const measureElement = useCallback(
    (node: Element | null) => {
      virtualizer.measureElement(node);
    },
    [virtualizer]
  );

  // -------------------------------------------------------------------------
  // Return
  // -------------------------------------------------------------------------

  return {
    virtualizer,
    virtualItems,
    totalSize,
    measureElement,
    scrollToBottom,
    scrollToIndex,
    scrollToPreviousUserMessage,
    isAtBottom: isAtBottomState,
    checkIsAtBottom,
    releaseBottomLock,
    rowIndexForVirtualItem,
    rowForVirtualItem,
  };
}
