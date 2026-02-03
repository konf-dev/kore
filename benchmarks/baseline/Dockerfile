FROM python:3.11-slim

# Install common tools that a standard agent would have
RUN apt-get update && apt-get install -y \
    curl \
    git \
    jq \
    nodejs \
    npm \
    wget \
    vim \
    && rm -rf /var/lib/apt/lists/*

# Install Python packages commonly used by AI agents
RUN pip install --no-cache-dir \
    requests \
    beautifulsoup4 \
    openai \
    httpx \
    pydantic

WORKDIR /workspace

# Copy the baseline agent
COPY baseline_agent.py /app/agent.py

# Run the agent
CMD ["python", "/app/agent.py"]
