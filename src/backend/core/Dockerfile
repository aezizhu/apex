FROM python:3.11-slim

RUN groupadd -r apex && useradd -r -g apex apex

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY src/ src/

RUN mkdir -p data/reports && chown -R apex:apex /app

USER apex

EXPOSE 8000

CMD ["python", "-m", "src.backend.core"]
