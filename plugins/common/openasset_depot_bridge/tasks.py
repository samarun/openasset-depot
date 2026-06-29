"""Small bounded worker used by UI hosts that require main-thread callbacks."""

from __future__ import annotations

import threading
from concurrent.futures import Future, ThreadPoolExecutor
from typing import Any, Callable, Optional, Set


Schedule = Callable[[Callable[[], None]], None]
Callback = Callable[[Any], None]
ErrorCallback = Callable[[Exception], None]


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
            with self._lock:
                self._futures.discard(done)
            try:
                result = done.result()
            except Exception as error:
                if on_error:
                    schedule(lambda error=error: on_error(error))
            else:
                if on_success:
                    schedule(lambda: on_success(result))

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
