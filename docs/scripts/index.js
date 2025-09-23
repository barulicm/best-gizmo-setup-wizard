function getClientPlatform() {
  const platform = navigator.platform.toLowerCase();
  const userAgent = navigator.userAgent.toLowerCase();
  let os = 'Unknown OS';
  let arch = 'Unknown Architecture';

  if (platform.includes('win')) {
    os = 'Windows';
    arch = userAgent.includes('wow64') || userAgent.includes('win64') ? 'x64' : 'x86';
  } else if (platform.includes('mac')) {
    os = 'macOS';
    arch = userAgent.includes('arm64') ? 'arm64' : 'x64';
  } else if (platform.includes('linux')) {
    os = 'Linux';
    arch = userAgent.includes('x86_64') ? 'x64' : 'arm64';
  } else if (platform.includes('android')) {
    os = 'Android';
    arch = userAgent.includes('arm64') ? 'arm64' : 'x86_64';
  } else if (platform.includes('iphone') || platform.includes('ipad')) {
    os = 'iOS';
    arch = userAgent.includes('arm64') ? 'arm64' : 'x86_64';
  }

  return `${os} ${arch}`;
}


function showDownloadLink() {
  var platform = getClientPlatform();
  switch (platform) {
    case 'Windows x64':
      document.getElementById('WindowsX64').style.display = 'block';
      break;
    case 'Linux x64':
      document.getElementById('LinuxX64').style.display = 'block';
      break;
    default:
      document.getElementById('UnsupportedPlatform').style.display = 'block';
      break;
  }
}


async function getLatestReleasePrefix() {
  let response = await fetch('https://api.github.com/repos/barulicm/best-gizmo-setup-wizard/releases/latest');
  if (response.ok) {
    let data = await response.json();
    if (data && data.status === 'success') {
      return "https://github.com/barulicm/best-gizmo-setup-wizard/releases/latest/download/";
    }
  }
  return null;
}


async function getNewestReleasePrefix() {
  let response = await fetch('https://api.github.com/repos/barulicm/best-gizmo-setup-wizard/releases');
  if (response.ok) {
    let releases = await response.json();
    releases.sort((a, b) => new Date(b.published_at) - new Date(a.published_at));
    const release_tag = releases[0].tag_name;
    if (release_tag) {
      return `https://github.com/barulicm/best-gizmo-setup-wizard/releases/download/${release_tag}/`;
    } else {
      console.error('Could not determine the download prefix from the newest release.');
      return null;
    }
  }
}


async function getDownloadPrefix() {
  const latest_prefix = await getLatestReleasePrefix();
  if (latest_prefix) {
    return latest_prefix;
  }
  return await getNewestReleasePrefix();
}


async function setDownloadLinks() {
  const prefix = await getDownloadPrefix();
  let links = document.getElementsByClassName('download-link');
  for(let link of links) {
    const href = link.getAttribute('href');
    if (href) {
      link.setAttribute('href', `${prefix}${href}`);
    }
  }
}


function onLoad() {
  showDownloadLink();
  setDownloadLinks();
}
