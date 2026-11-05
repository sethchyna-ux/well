part of 'generated.dart';

class ListMyDevicesVariablesBuilder {
  
  final FirebaseDataConnect _dataConnect;
  ListMyDevicesVariablesBuilder(this._dataConnect, );
  Deserializer<ListMyDevicesData> dataDeserializer = (dynamic json)  => ListMyDevicesData.fromJson(jsonDecode(json));
  
  Future<QueryResult<ListMyDevicesData, void>> execute({QueryFetchPolicy fetchPolicy = QueryFetchPolicy.preferCache}) {
    return ref().execute(fetchPolicy: fetchPolicy);
  }

  QueryRef<ListMyDevicesData, void> ref() {
    
    return _dataConnect.query("ListMyDevices", dataDeserializer, emptySerializer, null);
  }
}

@immutable
class ListMyDevicesDeviceSyncs {
  final String deviceName;
  final Timestamp lastSeen;
  ListMyDevicesDeviceSyncs.fromJson(dynamic json):
  
  deviceName = nativeFromJson<String>(json['deviceName']),
  lastSeen = Timestamp.fromJson(json['lastSeen']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyDevicesDeviceSyncs otherTyped = other as ListMyDevicesDeviceSyncs;
    return deviceName == otherTyped.deviceName && 
    lastSeen == otherTyped.lastSeen;
    
  }
  @override
  int get hashCode => Object.hashAll([deviceName.hashCode, lastSeen.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['deviceName'] = nativeToJson<String>(deviceName);
    json['lastSeen'] = lastSeen.toJson();
    return json;
  }

  ListMyDevicesDeviceSyncs({
    required this.deviceName,
    required this.lastSeen,
  });
}

@immutable
class ListMyDevicesData {
  final List<ListMyDevicesDeviceSyncs> deviceSyncs;
  ListMyDevicesData.fromJson(dynamic json):
  
  deviceSyncs = (json['deviceSyncs'] as List<dynamic>)
        .map((e) => ListMyDevicesDeviceSyncs.fromJson(e))
        .toList();
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final ListMyDevicesData otherTyped = other as ListMyDevicesData;
    return deviceSyncs == otherTyped.deviceSyncs;
    
  }
  @override
  int get hashCode => deviceSyncs.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['deviceSyncs'] = deviceSyncs.map((e) => e.toJson()).toList();
    return json;
  }

  ListMyDevicesData({
    required this.deviceSyncs,
  });
}

