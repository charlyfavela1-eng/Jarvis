#!/bin/bash
# JARVIS Daemon - Runs in background, survives terminal close
# Usage: ./jarvis-daemon.sh start|stop|status

JARVIS_DIR="/Users/michaelcaneyjr/Jarvis"
PID_FILE="/tmp/jarvis.pid"
LOG_FILE="/tmp/jarvis.log"

# Load environment
source ~/.zshrc 2>/dev/null

start_jarvis() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "JARVIS is already running (PID: $(cat $PID_FILE))"
        return
    fi

    echo "Starting JARVIS daemon..."

    cd "$JARVIS_DIR"
    source env/bin/activate
    export PYTHONPATH="$JARVIS_DIR:$PYTHONPATH"

    # Run JARVIS in background with nohup
    nohup python jarviscli >> "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"

    sleep 2
    if kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "JARVIS is now running (PID: $(cat $PID_FILE))"
        echo "Log file: $LOG_FILE"
    else
        echo "Failed to start JARVIS"
        rm -f "$PID_FILE"
    fi
}

stop_jarvis() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if kill -0 "$PID" 2>/dev/null; then
            echo "Stopping JARVIS (PID: $PID)..."
            kill "$PID"
            rm -f "$PID_FILE"
            echo "JARVIS stopped."
        else
            echo "JARVIS is not running (stale PID file)"
            rm -f "$PID_FILE"
        fi
    else
        echo "JARVIS is not running"
    fi
}

status_jarvis() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "JARVIS is running (PID: $(cat $PID_FILE))"
    else
        echo "JARVIS is not running"
    fi
}

case "$1" in
    start)
        start_jarvis
        ;;
    stop)
        stop_jarvis
        ;;
    status)
        status_jarvis
        ;;
    restart)
        stop_jarvis
        sleep 1
        start_jarvis
        ;;
    *)
        echo "Usage: $0 {start|stop|status|restart}"
        exit 1
        ;;
esac
