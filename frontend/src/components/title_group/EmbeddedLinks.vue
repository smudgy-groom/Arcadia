<template>
  <Tabs :value="0" size="small">
    <TabList>
      <Tab v-for="(link, i) in safeLinks" :key="link" :value="i">{{ `Trailer ${i + 1}` }}</Tab>
    </TabList>
    <TabPanels>
      <TabPanel v-for="(link, i) in safeLinks" :key="link" :value="i">
        <iframe
          v-if="isYoutubeLink(link) || unplayableLinks.includes(link)"
          style="width: 100%; aspect-ratio: 16 / 9; border: 0"
          :src="link"
          allowfullscreen
          referrerpolicy="strict-origin-when-cross-origin"
        />
        <video v-else style="width: 100%; aspect-ratio: 16 / 9" :src="link" controls preload="metadata" @error="unplayableLinks.push(link)" />
      </TabPanel>
    </TabPanels>
  </Tabs>
</template>

<script setup lang="ts">
import Tabs from 'primevue/tabs'
import TabList from 'primevue/tablist'
import Tab from 'primevue/tab'
import TabPanels from 'primevue/tabpanels'
import TabPanel from 'primevue/tabpanel'
import { computed, ref } from 'vue'
import { isYoutubeLink } from '@/services/helpers'

const props = defineProps<{
  links: string[]
}>()

// only http(s) URLs may reach the `src` of an iframe or video element: a `javascript:` URL
// assigned to an iframe's src executes in this document's origin
const isSafeMediaUrl = (url: string): boolean => {
  try {
    const { protocol } = new URL(url)
    return protocol === 'https:' || protocol === 'http:'
  } catch {
    return false
  }
}

const safeLinks = computed(() => (props.links ?? []).filter(isSafeMediaUrl))

// a link that is not a youtube one is played as a video file, and embedded in an iframe when it turns out not to be one
const unplayableLinks = ref<string[]>([])
</script>
