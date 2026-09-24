import os

# Set before crewai is imported: the tests run a real CrewAI agent against a scripted LLM.
os.environ["CREWAI_DISABLE_TELEMETRY"] = "true"
