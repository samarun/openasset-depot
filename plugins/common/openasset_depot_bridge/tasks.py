"""Small bounded worker used by UI hosts that require main-thread callbacks."""

from __future__ import annotations

import threading
from concurrent.futures import Future, ThreadPoolExecutor
from queue import Empty, SimpleQueue
from typing import Any, Callable, Optional, Set


Schedule = Callable[[Callable[[], None]], None]
Callback = Callable[[Any], None]
ErrorCallback = Callable[[Exception], None]


class CallbackQueue:
    """Thread-safe callbacks drained by hosts from their UI thread."""

    def __init__(self) -> None:
        self._callbacks: SimpleQueue[Callable[[], None]] = SimpleQueue()

    def schedule(self, callback: Callable[[], None]) -> None:
        self._callbacks.put(callback)

    def drain(self) -> int:
        count = 0
        while True:
            try:
                callback = self._callbacks.get_nowait()
            except Empty:
                return count
            callback()
            count += 1

    def clear(self) -> None:
        while True:
            try:
                self._callbacks.get_nowait()
            except Empty:
                return

    @property
    def empty(self) -> bool:
        return self._callbacks.empty()


class TaskRunner:
    def __init__(self, name: str = "openasset-host") -> None:
        self._executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix=name)
        self._futures: Set[Future[Any]] = set()
        self._lock = threading.Lock()
        self._closed = False

    def submit(
        self,
        operation: Callable[[], Any],
        *,
        schedule: Schedule,
        on_success: Optional[Callback] = None,
        on_error: Optional[ErrorCallback] = None,
    ) -> Future[Any]:
        with self._lock:
            if self._closed:
                raise RuntimeError("OpenAsset task runner is closed")
            future = self._executor.submit(operation)
            self._futures.add(future)

        def complete(done: Future[Any]) -> None:
            try:
                try:
                    result = done.result()
                except Exception as error:
                    if on_error:
                        schedule(lambda error=error: on_error(error))
                else:
                    if on_success:
                        schedule(lambda: on_success(result))
            finally:
                # Keep the future pending until its UI callback is queued. Hosts
                # that poll pending_count cannot otherwise miss a fast result.
                with self._lock:
                    self._futures.discard(done)

        future.add_done_callback(complete)
        return future

    def shutdown(self) -> None:
        with self._lock:
            self._closed = True
            futures = list(self._futures)
        for future in futures:
            future.cancel()
        self._executor.shutdown(wait=False)

    @property
    def pending_count(self) -> int:
        with self._lock:
            return len(self._futures)
