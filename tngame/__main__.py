from multiprocessing import Process
from pathlib import Path

from watchdog.events import FileSystemEventHandler
from watchdog.observers import Observer

if __name__ == '__main__':
    # Run the game server on a new process
    from . import telnet
    p = Process(target=telnet.run)
    p.start()

    # Listen when any python file in this directory is changed
    class MyHandler(FileSystemEventHandler):
        def on_modified(self, event):
            if not event.src_path.endswith(".py"):
                return

            global p
            print(f"File changed: {event.src_path}, reloading")

            # Stop the server
            p.terminate()

            # Re-import the module
            import importlib
            importlib.reload(telnet)

            # Run the server again
            p = Process(target=telnet.run)
            p.start()

    # Get script path
    script_path = Path(__file__).parent

    event_handler = MyHandler()
    observer = Observer()
    observer.schedule(event_handler, path=str(script_path.absolute()), recursive=False)
    observer.start()

    print("Watching for changes...")
    observer.join()
