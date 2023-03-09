FROM debian:sid-slim

# Install python
RUN apt-get update && apt-get install -y python3 python3-pip python3-venv && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Install requirements
RUN python3 -m venv venv \
    && ./venv/bin/pip install telnetlib3 hypy_utils rich

# Copy files
COPY requirements.txt relay.py tngame-rs/target/release/tngame-rs ./

ENV PYTHONUNBUFFERED=1
# Run
CMD ["./venv/bin/python3", "relay.py", "--port", "2323", "--bin", "./tngame-rs"]
