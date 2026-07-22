<script lang="ts">
  import { onMount } from "svelte";

  interface Message {
    id: string;
    role: "user" | "assistant";
    text: string;
    timestamp: Date;
  }

  let messages = $state<Message[]>([
    {
      id: "initial",
      role: "assistant",
      text: "Hello! I'm your MeraRAW AI Assistant. Tell me how you'd like to edit this photo (e.g. 'make it warmer', 'increase contrast', 'crop to square') or ask me questions about raw developing!",
      timestamp: new Date()
    }
  ]);

  let inputVal = $state("");
  let isTyping = $state(false);
  let scrollContainer = $state<HTMLDivElement | null>(null);

  function scrollToBottom() {
    if (scrollContainer) {
      setTimeout(() => {
        scrollContainer!.scrollTo({
          top: scrollContainer!.scrollHeight,
          behavior: "smooth"
        });
      }, 50);
    }
  }

  function handleSend() {
    if (!inputVal.trim() || isTyping) return;

    const userText = inputVal.trim();
    inputVal = "";

    // Add user message
    messages.push({
      id: Math.random().toString(),
      role: "user",
      text: userText,
      timestamp: new Date()
    });
    scrollToBottom();

    // Trigger typing response
    isTyping = true;
    setTimeout(() => {
      isTyping = false;
      const reply = generateResponse(userText);
      messages.push({
        id: Math.random().toString(),
        role: "assistant",
        text: reply,
        timestamp: new Date()
      });
      scrollToBottom();
    }, 1000);
  }

  function generateResponse(query: string): string {
    const text = query.toLowerCase();
    if (text.includes("warm") || text.includes("temp") || text.includes("sun")) {
      return "I can help with color temperature! Shifting the Temp slider to the right under 'Color Settings' will warm up the photo by adding amber tones.";
    }
    if (text.includes("crop") || text.includes("aspect") || text.includes("cut")) {
      return "To crop the image, select the 'Crop' tool (the second icon in the vertical selector) to reveal aspect ratio options and orientation presets.";
    }
    if (text.includes("contrast") || text.includes("bright") || text.includes("exposure")) {
      return "Let's refine the tonal range! Use the Exposure slider under 'Light Settings' to balance brightness, or increase Contrast to make shadows and highlights pop.";
    }
    if (text.includes("ai") || text.includes("mask") || text.includes("subject")) {
      return "MeraRAW includes automatic AI masking. Switch to the 'AI' panel (the magic wand icon) to run automatic subject and sky selections.";
    }
    if (text.includes("raw") || text.includes("cr3") || text.includes("nef")) {
      return "MeraRAW decodes RAW sensor data directly using LibRaw. This preserves maximum dynamic range, letting you recover shadow details and highlight clipping.";
    }
    return "Understood! I've simulated that adjustment on the active RAW image. You can use the sliders in the 'Edit' panel to fine-tune the final look.";
  }

  onMount(() => {
    scrollToBottom();
  });
</script>

<div class="flex h-full flex-col min-h-0">
  <div class="flex items-center justify-between mb-[10px] mt-[4px] px-[8px] shrink-0">
    <span class="text-[12px] font-semibold text-white/90">AI Assistant</span>
    <span class="text-[8px] px-2 py-0.5 rounded bg-white/[0.08] text-white/50">RAW Copilot</span>
  </div>

  <!-- Messages List -->
  <div
    bind:this={scrollContainer}
    class="flex-1 overflow-y-auto px-[6px] py-[4px] flex flex-col gap-[10px] min-h-0 custom-scrollbar mb-3"
  >
    {#each messages as msg (msg.id)}
      <div class="flex flex-col gap-[2px] {msg.role === 'user' ? 'items-end' : 'items-start'}">
        <!-- Role Label -->
        <span class="text-[8px] text-white/30 px-1 capitalize">{msg.role}</span>
        
        <!-- Message bubble -->
        <div
          class="max-w-[85%] rounded-[12px] p-[8px] text-[10px] leading-relaxed break-words
            {msg.role === 'user'
              ? 'bg-[#3b82f6] text-white rounded-tr-none'
              : 'bg-white/[0.04] border border-white/[0.03] text-white/80 rounded-tl-none'
            }"
        >
          {msg.text}
        </div>
      </div>
    {/each}

    {#if isTyping}
      <div class="flex flex-col gap-[2px] items-start">
        <span class="text-[8px] text-white/30 px-1">Assistant</span>
        <div class="bg-white/[0.04] border border-white/[0.03] text-white/50 rounded-[12px] rounded-tl-none p-[8px] text-[10px] flex gap-[3px] items-center">
          <span class="animate-bounce">●</span>
          <span class="animate-bounce [animation-delay:0.2s]">●</span>
          <span class="animate-bounce [animation-delay:0.4s]">●</span>
        </div>
      </div>
    {/if}
  </div>

  <!-- Message Input Area -->
  <div class="shrink-0 px-[6px] pb-[8px] border-t border-white/[0.04] pt-[10px]">
    <div class="chat-input-wrapper flex items-center gap-[6px] bg-white/[0.02] border border-white/[0.06] rounded-[10px] p-[4px]">
      <input
        type="text"
        bind:value={inputVal}
        placeholder="Type a message..."
        class="chat-input flex-1 h-[26px] bg-transparent text-[10px] text-white placeholder:text-white/35 outline-none px-2"
        onkeydown={(e) => e.key === "Enter" && handleSend()}
      />
      <button
        onclick={handleSend}
        disabled={!inputVal.trim() || isTyping}
        class="chat-send-btn h-[26px] px-3 bg-white text-black font-semibold text-[10px] rounded-[8px] flex items-center justify-center transition-all disabled:opacity-30 enabled:hover:bg-white/90 cursor-pointer"
      >
        Send
      </button>
    </div>
  </div>
</div>

<style>
  /* Custom scrollbar matching dark design language */
  .custom-scrollbar::-webkit-scrollbar {
    width: 4px;
  }
  
  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }
  
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 99px;
  }
</style>
